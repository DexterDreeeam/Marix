use std::fs::{File, OpenOptions};
use std::io::Write;
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
const FALLBACK_LOG_FILE_NAME: &str = "marix.log";

static LOGGER: Logger = Logger::new();

pub struct Logger {
    configuration: Mutex<()>,
    source: OnceLock<LogSource>,
    sink: OnceLock<Mutex<Sink>>,
    session_id: RwLock<Option<uuid::Uuid>>,
}

impl Logger {
    /// Records local and remote telemetry in the host redb store.
    pub fn host() -> Result<(), LoggingError> {
        LOGGER.configure(LogSource::Server, || {
            let config = Config::load().map_err(LoggingError::Config)?;
            let store = Store::open(&config)?;
            Ok(Sink::Host(store))
        })?;
        thread::spawn(accept_loop);
        Ok(())
    }

    /// Configures this process to stream telemetry to the server, falling back
    /// to a JSON Lines file beside the executable when unavailable.
    ///
    /// The connect is retried briefly because the server may not be listening
    /// yet or may be rebinding between accepted connections.
    pub fn connect(source: LogSource) -> Result<(), LoggingError> {
        LOGGER.configure(source, || {
            let fallback_path = Self::fallback_log_path()?;
            match std::fs::remove_file(&fallback_path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(LoggingError::Io(format!(
                        "failed to remove previous fallback log '{}': {error}",
                        fallback_path.display()
                    )));
                }
            }
            match Self::connect_sender() {
                Ok(sender) => Ok(Sink::Remote {
                    sender,
                    fallback_path,
                }),
                Err(_) => Ok(Sink::File(LogFile::create(&fallback_path)?)),
            }
        })
    }

    pub fn set_id(id: uuid::Uuid) {
        *LOGGER
            .session_id
            .write()
            .unwrap_or_else(|error| error.into_inner()) = Some(id);
    }

    pub fn log(message: impl Into<String>) {
        LOGGER.emit(LogTag::Info, message.into());
    }

    pub fn warning(message: impl Into<String>) {
        LOGGER.emit(LogTag::Warning, message.into());
    }

    pub fn error(message: impl Into<String>) {
        LOGGER.emit(LogTag::Error, message.into());
    }

    pub fn debug(message: impl Into<String>) {
        LOGGER.emit(LogTag::Debug, message.into());
    }

    /// Returns every telemetry message recorded by this process's host store,
    /// in ascending record-insertion order. Only available when this process
    /// is hosting a store via [`Logger::host`]. Used by
    /// `crate::logging::query` to answer session/log queries.
    pub(super) fn local_log() -> Result<Vec<LogMessage>, LoggingError> {
        let Some(sink) = LOGGER.sink.get() else {
            return Err(LoggingError::NotHosting);
        };
        let sink = sink
            .lock()
            .map_err(|error| LoggingError::Io(error.to_string()))?;
        host_store(Some(&sink))?.read_all()
    }
}

pub(super) struct Store {
    database: Database,
    next_id: AtomicU64,
}

impl Store {
    pub(super) fn open_at(path: &Path) -> Result<Self, LoggingError> {
        let database =
            Database::create(path).map_err(|error| LoggingError::Database(error.to_string()))?;
        let next_id = Self::stored_count(&database)?;
        Ok(Self {
            database,
            next_id: AtomicU64::new(next_id),
        })
    }

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
            .set(Mutex::new(sink))
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
        let Some(sink) = self.sink.get() else {
            return Ok(());
        };
        let mut sink = sink
            .lock()
            .map_err(|error| LoggingError::Io(error.to_string()))?;
        let mut replacement = None;
        let result = match &mut *sink {
            Sink::Host(store) => {
                message.stamp_arrival();
                store.record(&message)
            }
            Sink::File(file) => file.append(&message),
            Sink::Remote {
                sender,
                fallback_path,
            } => {
                if sender.try_send(message.clone()).is_ok() {
                    Ok(())
                } else {
                    let mut file = LogFile::create(fallback_path)?;
                    file.append(&message)?;
                    replacement = Some(Sink::File(file));
                    Ok(())
                }
            }
        };
        if let Some(new_sink) = replacement {
            *sink = new_sink;
        }
        result
    }

    fn fallback_log_path() -> Result<PathBuf, LoggingError> {
        let executable = std::env::current_exe().map_err(|error| {
            LoggingError::Io(format!(
                "failed to resolve current executable path for fallback log: {error}"
            ))
        })?;
        let parent = executable
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .ok_or_else(|| {
                LoggingError::Io(format!(
                    "current executable path '{}' has no parent directory for fallback log",
                    executable.display()
                ))
            })?;
        Ok(parent.join(FALLBACK_LOG_FILE_NAME))
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
        let Some(sink) = self.sink.get() else {
            return Err(LoggingError::NotHosting);
        };
        let sink = sink
            .lock()
            .map_err(|error| LoggingError::Io(error.to_string()))?;
        match &*sink {
            Sink::Host(store) => store.record(&message),
            Sink::Remote { .. } | Sink::File(_) => Err(LoggingError::NotHosting),
        }
    }
}

enum Sink {
    Host(Store),
    Remote {
        sender: NetSender<LogMessage>,
        fallback_path: PathBuf,
    },
    File(LogFile),
}

struct LogFile {
    file: File,
}

impl LogFile {
    fn create(path: &Path) -> Result<Self, LoggingError> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)
            .map_err(|error| {
                LoggingError::Io(format!(
                    "failed to create fallback log '{}': {error}",
                    path.display()
                ))
            })?;
        Ok(Self { file })
    }

    fn append(&mut self, message: &LogMessage) -> Result<(), LoggingError> {
        let bytes = serde_json::to_vec(message)
            .map_err(|error| LoggingError::Serialization(error.to_string()))?;
        self.file.write_all(&bytes)?;
        self.file.write_all(b"\n")?;
        self.file.flush()?;
        Ok(())
    }
}

fn host_store(sink: Option<&Sink>) -> Result<&Store, LoggingError> {
    match sink {
        Some(Sink::Host(store)) => Ok(store),
        _ => Err(LoggingError::NotHosting),
    }
}

impl Store {
    fn open(config: &Config) -> Result<Self, LoggingError> {
        let directory = Self::database_directory(config);
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

    fn database_directory(config: &Config) -> PathBuf {
        let base = config
            .runtime
            .marix_path_server
            .as_deref()
            .unwrap_or(config.runtime.marix_path.as_str());
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

/// Accepts telemetry connections and gives each one a receive worker.
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
