use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock, RwLock};
use std::thread;
use std::time::Duration;

use crate::config::Config;
use crate::external::redb::{
    Database, ReadableDatabase, ReadableTable, ReadableTableMetadata, TableDefinition,
};
use crate::external::{serde_json, uuid};
use crate::logging::{LogMessage, LogSource, LogTag, LoggingError};
use crate::structure::{ChannelEndpoint, NetReceiver, NetSender, accept_channel, connect_channel};

const TELEMETRY_TABLE: TableDefinition<u64, &[u8]> = TableDefinition::new("telemetry");
const TELEMETRY_FILE_NAME: &str = "telemetry.redb";

static LOGGER: Logger = Logger::new();

pub struct Logger {
    configuration: Mutex<()>,
    source: OnceLock<LogSource>,
    sink: OnceLock<Sink>,
    session_id: RwLock<Option<uuid::Uuid>>,
}

impl Logger {
    /// Records telemetry in a local database and accepts telemetry from other
    /// processes.
    ///
    /// The bind address and handshake token are resolved from configuration
    /// by [`accept_channel`]; each accepted client connection is served by its
    /// own receive worker, so telemetry from multiple processes is recorded
    /// concurrently.
    pub fn host() -> Result<(), LoggingError> {
        LOGGER.configure(LogSource::Server, || {
            let config = Config::load().map_err(LoggingError::Config)?;
            let store = Store::open(&config, StoreRole::Server)?;
            Ok(Sink::Host(store))
        })?;
        thread::spawn(accept_loop);
        Ok(())
    }

    /// Configures this process to stream telemetry to the server or record it
    /// in a local database, according to the logging configuration.
    ///
    /// The connect address and handshake token are resolved from
    /// configuration by [`connect_channel`]. The connect is retried a few
    /// times, since the server may not yet be listening or may be briefly
    /// rebinding between accepted connections.
    pub fn connect(source: LogSource) -> Result<(), LoggingError> {
        LOGGER.configure(source, || {
            let config = Config::load().map_err(LoggingError::Config)?;
            if config.logging.remote {
                let sender = Self::connect_sender()?;
                Ok(Sink::Remote(Mutex::new(sender)))
            } else {
                Ok(Sink::Local(Store::open(&config, StoreRole::Runtime)?))
            }
        })
    }

    /// Sets the session identifier attached to future telemetry messages.
    pub fn set_id(id: uuid::Uuid) {
        *LOGGER
            .session_id
            .write()
            .unwrap_or_else(|error| error.into_inner()) = Some(id);
    }

    /// Emits an info-tagged telemetry message.
    pub fn log(message: impl Into<String>) {
        LOGGER.emit(LogTag::Info, message.into());
    }

    /// Emits a warning-tagged telemetry message.
    pub fn warning(message: impl Into<String>) {
        LOGGER.emit(LogTag::Warning, message.into());
    }

    /// Emits an error-tagged telemetry message.
    pub fn error(message: impl Into<String>) {
        LOGGER.emit(LogTag::Error, message.into());
    }

    /// Emits a debug-tagged telemetry message.
    pub fn debug(message: impl Into<String>) {
        LOGGER.emit(LogTag::Debug, message.into());
    }

    /// Returns every telemetry message recorded by this process's host
    /// store, in ascending record-insertion order. Only available when this
    /// process is hosting a store via [`Logger::host`] — a
    /// [`Logger::connect`] runtime-local store is deliberately not
    /// queryable. Used by `crate::logging::query` to answer session/log
    /// queries.
    pub(super) fn local_log() -> Result<Vec<LogMessage>, LoggingError> {
        host_store(LOGGER.sink.get())?.read_all()
    }
}

/// A telemetry store backed by a redb database, used both to record
/// incoming messages and to answer read-only session/log queries.
pub(super) struct Store {
    database: Database,
    next_id: AtomicU64,
}

impl Store {
    /// Opens the redb database at `path` for persistent append, creating it
    /// when absent and restoring the next record id from the existing table.
    pub(super) fn open_at(path: &Path) -> Result<Self, LoggingError> {
        let database =
            Database::create(path).map_err(|error| LoggingError::Database(error.to_string()))?;
        let next_id = Self::stored_count(&database)?;
        Ok(Self {
            database,
            next_id: AtomicU64::new(next_id),
        })
    }

    /// Reads every recorded message in a single read transaction, in
    /// ascending record-id (insertion) order.
    pub(super) fn read_all(&self) -> Result<Vec<LogMessage>, LoggingError> {
        let read_txn = self
            .database
            .begin_read()
            .map_err(|error| LoggingError::Database(error.to_string()))?;
        let table = read_txn
            .open_table(TELEMETRY_TABLE)
            .map_err(|error| LoggingError::Database(error.to_string()))?;
        let mut messages = Vec::new();
        for entry in table
            .iter()
            .map_err(|error| LoggingError::Database(error.to_string()))?
        {
            let (_key, value) = entry.map_err(|error| LoggingError::Database(error.to_string()))?;
            let message: LogMessage = serde_json::from_slice(value.value())
                .map_err(|error| LoggingError::Serialization(error.to_string()))?;
            messages.push(message);
        }
        Ok(messages)
    }

    pub(super) fn record(&self, message: &LogMessage) -> Result<(), LoggingError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let bytes = serde_json::to_vec(message)
            .map_err(|error| LoggingError::Serialization(error.to_string()))?;
        let write = self
            .database
            .begin_write()
            .map_err(|error| LoggingError::Database(error.to_string()))?;
        {
            let mut table = write
                .open_table(TELEMETRY_TABLE)
                .map_err(|error| LoggingError::Database(error.to_string()))?;
            table
                .insert(id, bytes.as_slice())
                .map_err(|error| LoggingError::Database(error.to_string()))?;
        }
        write
            .commit()
            .map_err(|error| LoggingError::Database(error.to_string()))?;
        Ok(())
    }
}

// -- Private -- //

impl Logger {
    const fn new() -> Self {
        Self {
            configuration: Mutex::new(()),
            source: OnceLock::new(),
            sink: OnceLock::new(),
            session_id: RwLock::new(None),
        }
    }

    fn configure(
        &self,
        source: LogSource,
        create_sink: impl FnOnce() -> Result<Sink, LoggingError>,
    ) -> Result<(), LoggingError> {
        let _configuration = self
            .configuration
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if self.source.get().is_some() || self.sink.get().is_some() {
            return Err(LoggingError::AlreadyConfigured);
        }

        let sink = create_sink()?;
        self.source
            .set(source)
            .map_err(|_| LoggingError::AlreadyConfigured)?;
        self.sink
            .set(sink)
            .map_err(|_| LoggingError::AlreadyConfigured)
    }

    fn emit(&self, tag: LogTag, message: String) {
        if let Err(error) = self.telemetry(tag, message) {
            Self::report_error(error);
        }
    }

    fn report_error(error: LoggingError) {
        eprintln!("marix logger failed: {error}");
    }

    fn telemetry(&self, tag: LogTag, message: String) -> Result<(), LoggingError> {
        let Some(source) = self.source.get().copied() else {
            return Ok(());
        };
        let mut message = LogMessage::new(tag, message);
        message.source = source;
        message.session_id = *self
            .session_id
            .read()
            .unwrap_or_else(|error| error.into_inner());
        match self.sink.get() {
            Some(Sink::Host(_)) | Some(Sink::Local(_)) => self.record(message),
            Some(Sink::Remote(sender)) => {
                let guard = sender
                    .lock()
                    .map_err(|error| LoggingError::Io(error.to_string()))?;
                guard
                    .try_send(message)
                    .map_err(|error| LoggingError::Channel(error.to_string()))?;
                Ok(())
            }
            None => Ok(()),
        }
    }

    /// Connects the telemetry channel, retrying a few times so a not-yet-ready
    /// or briefly rebinding server does not permanently disable telemetry.
    fn connect_sender() -> Result<NetSender<LogMessage>, LoggingError> {
        const MAX_ATTEMPTS: usize = 5;
        const RETRY_BACKOFF: Duration = Duration::from_millis(200);
        let mut last_error = None;
        for attempt in 0..MAX_ATTEMPTS {
            match connect_channel::<LogMessage>(ChannelEndpoint::Telemetry) {
                Ok((net_tx, _net_rx)) => return Ok(net_tx),
                Err(error) => {
                    last_error = Some(error);
                    if attempt + 1 < MAX_ATTEMPTS {
                        thread::sleep(RETRY_BACKOFF);
                    }
                }
            }
        }
        Err(LoggingError::Channel(
            last_error
                .map(|error| format!("{error:?}"))
                .unwrap_or_else(|| "telemetry channel connect failed".to_owned()),
        ))
    }

    fn record(&self, mut message: LogMessage) -> Result<(), LoggingError> {
        message.stamp_arrival();
        match self.sink.get() {
            Some(Sink::Host(store)) | Some(Sink::Local(store)) => store.record(&message),
            _ => Err(LoggingError::NotHosting),
        }
    }
}

enum StoreRole {
    Runtime,
    Server,
}

enum Sink {
    Host(Store),
    Local(Store),
    Remote(Mutex<NetSender<LogMessage>>),
}

/// Selects the queryable store from a resolved sink: only a
/// [`Sink::Host`] store (installed by [`Logger::host`]) is queryable. A
/// [`Sink::Local`] store (installed by [`Logger::connect`] when
/// `logging.remote` is `false`), a [`Sink::Remote`] sink, and an
/// unconfigured logger all report [`LoggingError::NotHosting`].
fn host_store(sink: Option<&Sink>) -> Result<&Store, LoggingError> {
    match sink {
        Some(Sink::Host(store)) => Ok(store),
        _ => Err(LoggingError::NotHosting),
    }
}

impl Store {
    /// Opens the role-specific persistent `telemetry.redb`. When the fixed
    /// database does not exist, legacy timestamped databases are imported
    /// atomically in filename and record order without deleting them.
    fn open(config: &Config, role: StoreRole) -> Result<Self, LoggingError> {
        let directory = Self::database_directory(config, role);
        Self::open_directory(&directory)
    }

    fn open_directory(directory: &Path) -> Result<Self, LoggingError> {
        std::fs::create_dir_all(&directory)?;
        let path = directory.join(TELEMETRY_FILE_NAME);
        if path.exists() {
            return Self::open_at(&path);
        }

        let legacy_paths = Self::legacy_database_paths(directory)?;
        if legacy_paths.is_empty() {
            return Self::open_at(&path);
        }
        Self::migrate_legacy(&path, &legacy_paths)
    }

    /// Directory that holds telemetry databases under the process's resolved
    /// runtime path. Servers prefer their role-specific runtime path.
    fn database_directory(config: &Config, role: StoreRole) -> PathBuf {
        let base = match role {
            StoreRole::Runtime => config.runtime.marix_path.as_str(),
            StoreRole::Server => config
                .runtime
                .marix_path_server
                .as_deref()
                .unwrap_or(config.runtime.marix_path.as_str()),
        };
        PathBuf::from(base).join("log")
    }

    fn legacy_database_paths(directory: &Path) -> Result<Vec<PathBuf>, LoggingError> {
        let mut paths = Vec::new();
        for entry in std::fs::read_dir(directory)? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let file_name = entry.file_name();
            let Some(file_name) = file_name.to_str() else {
                continue;
            };
            if file_name.starts_with("telemetry-") && file_name.ends_with(".redb") {
                paths.push(entry.path());
            }
        }
        paths.sort();
        Ok(paths)
    }

    fn migrate_legacy(path: &Path, legacy_paths: &[PathBuf]) -> Result<Self, LoggingError> {
        let temporary_path = path.with_file_name(format!(
            "{TELEMETRY_FILE_NAME}.migrating-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4(),
        ));
        let migration = (|| {
            let migrated = Self::open_at(&temporary_path)?;
            for legacy_path in legacy_paths {
                for message in Self::read_at(legacy_path)? {
                    migrated.record(&message)?;
                }
            }
            drop(migrated);
            std::fs::rename(&temporary_path, path)?;
            Self::open_at(path)
        })();

        match migration {
            Ok(store) => Ok(store),
            Err(error) => match std::fs::remove_file(&temporary_path) {
                Ok(()) => Err(error),
                Err(cleanup) if cleanup.kind() == std::io::ErrorKind::NotFound => Err(error),
                Err(cleanup) => Err(LoggingError::Io(format!(
                    "{error}; failed to remove migration database: {cleanup}",
                ))),
            },
        }
    }

    fn read_at(path: &Path) -> Result<Vec<LogMessage>, LoggingError> {
        let database =
            Database::open(path).map_err(|error| LoggingError::Database(error.to_string()))?;
        let store = Self {
            database,
            next_id: AtomicU64::new(0),
        };
        store.read_all()
    }

    fn stored_count(database: &Database) -> Result<u64, LoggingError> {
        let write = database
            .begin_write()
            .map_err(|error| LoggingError::Database(error.to_string()))?;
        let count = {
            let table = write
                .open_table(TELEMETRY_TABLE)
                .map_err(|error| LoggingError::Database(error.to_string()))?;
            table
                .len()
                .map_err(|error| LoggingError::Database(error.to_string()))?
        };
        write
            .commit()
            .map_err(|error| LoggingError::Database(error.to_string()))?;
        Ok(count)
    }
}

/// Accepts telemetry connections in a loop, serving each accepted connection
/// with its own receive worker. A failed accept (e.g. a bind race between
/// connections or a rejected handshake) is retried after a short pause rather
/// than aborting the loop, since telemetry is best-effort.
fn accept_loop() {
    loop {
        match accept_channel::<LogMessage>(ChannelEndpoint::Telemetry) {
            Ok((_net_tx, net_rx)) => {
                thread::spawn(move || worker(net_rx));
            }
            Err(_) => {
                thread::sleep(Duration::from_millis(200));
            }
        }
    }
}

/// Records every telemetry message received on a single accepted connection,
/// stamping arrival time and writing it to the local store.
fn worker(mut net_rx: NetReceiver<LogMessage>) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(_) => return,
    };
    runtime.block_on(async move {
        while let Ok(Some(message)) = net_rx.recv().await {
            if let Err(error) = LOGGER.record(message) {
                Logger::report_error(error);
            }
        }
    });
}

#[cfg(test)]
#[path = "logger_tests.rs"]
mod tests;
