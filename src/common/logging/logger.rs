use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::config::Config;
use crate::external::redb::{Database, ReadableTableMetadata, TableDefinition};
use crate::external::serde_json;
use crate::logging::{LogMessage, LogTag, LoggingError};
use crate::structure::{ChannelEndpoint, NetReceiver, NetSender, accept_channel, connect_channel};

const TELEMETRY_TABLE: TableDefinition<u64, &[u8]> = TableDefinition::new("telemetry");

static LOGGER: Logger = Logger::new();

pub struct Logger {
    sink: OnceLock<Sink>,
}

impl Logger {
    /// Records this server process's telemetry in a local database and, when
    /// remote logging is configured, accepts telemetry from other processes.
    ///
    /// The bind address and handshake token are resolved from configuration
    /// by [`accept_channel`]; each accepted client connection is served by its
    /// own receive worker, so telemetry from multiple processes is recorded
    /// concurrently.
    pub fn host() -> Result<(), LoggingError> {
        let config = Config::load().map_err(LoggingError::Config)?;
        let store = Store::open(&config, StoreRole::Server)?;
        LOGGER
            .sink
            .set(Sink::Local(store))
            .map_err(|_| LoggingError::AlreadyConfigured)?;
        if config.logging.remote {
            thread::spawn(accept_loop);
        }
        Ok(())
    }

    /// Configures this process to stream telemetry to the server or record it
    /// in a local database, according to the logging configuration.
    ///
    /// The connect address and handshake token are resolved from
    /// configuration by [`connect_channel`]. The connect is retried a few
    /// times, since the server may not yet be listening or may be briefly
    /// rebinding between accepted connections.
    pub fn connect() -> Result<(), LoggingError> {
        let config = Config::load().map_err(LoggingError::Config)?;
        let sink = if config.logging.remote {
            let sender = Self::connect_sender()?;
            Sink::Remote(Mutex::new(sender))
        } else {
            Sink::Local(Store::open(&config, StoreRole::Runtime)?)
        };
        LOGGER
            .sink
            .set(sink)
            .map_err(|_| LoggingError::AlreadyConfigured)?;
        Ok(())
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
}

// -- Private -- //

impl Logger {
    const fn new() -> Self {
        Self {
            sink: OnceLock::new(),
        }
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
        let message = LogMessage::new(tag, message);
        match self.sink.get() {
            Some(Sink::Local(_)) => self.record(message),
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
            Some(Sink::Local(store)) => store.record(&message),
            _ => Err(LoggingError::NotHosting),
        }
    }
}

enum StoreRole {
    Runtime,
    Server,
}

enum Sink {
    Local(Store),
    Remote(Mutex<NetSender<LogMessage>>),
}

struct Store {
    database: Database,
    next_id: AtomicU64,
}

impl Store {
    fn open(config: &Config, role: StoreRole) -> Result<Self, LoggingError> {
        let directory = Self::database_directory(config, role);
        std::fs::create_dir_all(&directory)?;
        let path = directory.join(Self::database_file_name());
        let database =
            Database::create(&path).map_err(|error| LoggingError::Database(error.to_string()))?;
        let next_id = Self::stored_count(&database)?;
        Ok(Self {
            database,
            next_id: AtomicU64::new(next_id),
        })
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

    /// A fresh timestamped database file name, so each server start records
    /// into its own database rather than appending to a shared one. The
    /// process ID prevents concurrent local processes from sharing a file.
    fn database_file_name() -> String {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs())
            .unwrap_or(0);
        format!(
            "telemetry-{}-{}.redb",
            Self::format_timestamp(seconds),
            std::process::id()
        )
    }

    /// Formats a Unix second count as a `YYYYMMDD-HHMMSS` UTC stamp without
    /// pulling in an external date/time dependency.
    fn format_timestamp(seconds: u64) -> String {
        let time_of_day = seconds % 86_400;
        let hour = time_of_day / 3_600;
        let minute = (time_of_day % 3_600) / 60;
        let second = time_of_day % 60;
        let (year, month, day) = Self::civil_from_days((seconds / 86_400) as i64);
        format!("{year:04}{month:02}{day:02}-{hour:02}{minute:02}{second:02}")
    }

    /// Converts a day count since the Unix epoch into a `(year, month, day)`
    /// Gregorian date using Howard Hinnant's civil-from-days algorithm.
    fn civil_from_days(days: i64) -> (i64, u32, u32) {
        let z = days + 719_468;
        let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
        let day_of_era = z - era * 146_097;
        let year_of_era =
            (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
        let year = year_of_era + era * 400;
        let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
        let month_position = (5 * day_of_year + 2) / 153;
        let day = (day_of_year - (153 * month_position + 2) / 5 + 1) as u32;
        let month = (if month_position < 10 {
            month_position + 3
        } else {
            month_position - 9
        }) as u32;
        let year = year + if month <= 2 { 1 } else { 0 };
        (year, month, day)
    }

    fn record(&self, message: &LogMessage) -> Result<(), LoggingError> {
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
