use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread;

use crate::config::Config;
use crate::external::redb::{Database, ReadableTableMetadata, TableDefinition};
use crate::external::serde_json;
use crate::logging::{LogMessage, LogTag, LoggingError};

const TELEMETRY_TABLE: TableDefinition<u64, &[u8]> = TableDefinition::new("telemetry");

static LOGGER: Logger = Logger::new();

pub struct Logger {
    sink: OnceLock<Sink>,
}

impl Logger {
    /// Starts the telemetry logger server on the given port and records this
    /// process's own telemetry directly to the local database. Agent-only.
    pub fn host(port: u16) -> Result<(), LoggingError> {
        let store = Store::open()?;
        LOGGER
            .sink
            .set(Sink::Local(store))
            .map_err(|_| LoggingError::AlreadyConfigured)?;
        let listener = TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], port)))?;
        thread::spawn(move || spawn_worker(listener));
        Ok(())
    }

    /// Connects this process to a telemetry logger server, blocking until the
    /// connection is established. Later telemetry is streamed to that server.
    pub fn connect(socket: SocketAddr) -> Result<(), LoggingError> {
        let stream = TcpStream::connect(socket)?;
        LOGGER
            .sink
            .set(Sink::Remote(Mutex::new(stream)))
            .map_err(|_| LoggingError::AlreadyConfigured)?;
        Ok(())
    }

    /// Emits an info-tagged telemetry message.
    pub fn log(message: impl Into<String>) -> Result<(), LoggingError> {
        LOGGER.telemetry(LogTag::Info, message.into())
    }

    /// Emits a warning-tagged telemetry message.
    pub fn warning(message: impl Into<String>) -> Result<(), LoggingError> {
        LOGGER.telemetry(LogTag::Warning, message.into())
    }

    /// Emits an error-tagged telemetry message.
    pub fn error(message: impl Into<String>) -> Result<(), LoggingError> {
        LOGGER.telemetry(LogTag::Error, message.into())
    }

    /// Emits a debug-tagged telemetry message.
    pub fn debug(message: impl Into<String>) -> Result<(), LoggingError> {
        LOGGER.telemetry(LogTag::Debug, message.into())
    }
}

// -- Private -- //

impl Logger {
    const fn new() -> Self {
        Self {
            sink: OnceLock::new(),
        }
    }

    fn telemetry(&self, tag: LogTag, message: String) -> Result<(), LoggingError> {
        let message = LogMessage::new(tag, message);
        match self.sink.get() {
            Some(Sink::Local(_)) => self.record(message),
            Some(Sink::Remote(stream)) => send_message(stream, &message),
            None => Ok(()),
        }
    }

    fn record(&self, mut message: LogMessage) -> Result<(), LoggingError> {
        message.stamp_arrival();
        match self.sink.get() {
            Some(Sink::Local(store)) => store.record(&message),
            _ => Err(LoggingError::NotHosting),
        }
    }
}

enum Sink {
    Local(Store),
    Remote(Mutex<TcpStream>),
}

struct Store {
    database: Database,
    next_id: AtomicU64,
}

impl Store {
    fn open() -> Result<Self, LoggingError> {
        let config = Config::load().map_err(LoggingError::Config)?;
        let path = PathBuf::from(config.telemetry.database_path);
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        let database =
            Database::create(&path).map_err(|error| LoggingError::Database(error.to_string()))?;
        let next_id = Self::stored_count(&database)?;
        Ok(Self {
            database,
            next_id: AtomicU64::new(next_id),
        })
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

fn spawn_worker(listener: TcpListener) {
    loop {
        match listener.accept() {
            Ok((stream, _address)) => {
                thread::spawn(move || run_worker(stream));
            }
            Err(_) => continue,
        }
    }
}

fn run_worker(mut stream: TcpStream) {
    while let Ok(Some(message)) = read_message(&mut stream) {
        let _ = LOGGER.record(message);
    }
}

fn send_message(stream: &Mutex<TcpStream>, message: &LogMessage) -> Result<(), LoggingError> {
    let bytes = serde_json::to_vec(message)
        .map_err(|error| LoggingError::Serialization(error.to_string()))?;
    let length = u32::try_from(bytes.len())
        .map_err(|_| LoggingError::Serialization("telemetry message too large".to_owned()))?;
    let mut guard = stream
        .lock()
        .map_err(|error| LoggingError::Io(error.to_string()))?;
    guard.write_all(&length.to_be_bytes())?;
    guard.write_all(&bytes)?;
    guard.flush()?;
    Ok(())
}

fn read_message(stream: &mut TcpStream) -> Result<Option<LogMessage>, LoggingError> {
    let mut length_buffer = [0_u8; 4];
    match stream.read_exact(&mut length_buffer) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(LoggingError::Io(error.to_string())),
    }
    let length = u32::from_be_bytes(length_buffer) as usize;
    let mut buffer = vec![0_u8; length];
    stream.read_exact(&mut buffer)?;
    let message = serde_json::from_slice(&buffer)
        .map_err(|error| LoggingError::Serialization(error.to_string()))?;
    Ok(Some(message))
}
