use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, RwLock};
use std::thread;
use std::time::Duration;

use crate::config::Config;
use crate::external::{serde_json, uuid};
use crate::logging::store::{HostStore, Store};
use crate::logging::{LogLevel, LogMessage, LogSource, LoggingError};
use crate::structure::{ChannelEndpoint, NetReceiver, NetSender, accept_channel, connect_channel};

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
            Ok(Sink::Host(HostStore::new(store)?))
        })?;
        thread::spawn(accept_loop);
        Ok(())
    }

    /// Configures this process to stream telemetry to the server.
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
            Self::remote_sink(fallback_path, Self::connect_sender)
        })
    }

    pub fn set_id(id: uuid::Uuid) {
        *LOGGER
            .session_id
            .write()
            .unwrap_or_else(|error| error.into_inner()) = Some(id);
    }

    pub fn log(message: impl Into<String>) {
        LOGGER.emit(LogLevel::Info, message.into());
    }

    pub fn warning(message: impl Into<String>) {
        LOGGER.emit(LogLevel::Warning, message.into());
    }

    pub fn error(message: impl Into<String>) {
        LOGGER.emit(LogLevel::Error, message.into());
    }

    pub fn debug(message: impl Into<String>) {
        LOGGER.emit(LogLevel::Debug, message.into());
    }

    pub fn flush() -> Result<(), LoggingError> {
        Self::host_sink()?.flush()
    }

    pub(super) fn host_sink() -> Result<HostStore, LoggingError> {
        let Some(sink) = LOGGER.sink.get() else {
            return Err(LoggingError::NotHosting);
        };
        let sink = sink
            .lock()
            .map_err(|error| LoggingError::Io(error.to_string()))?;
        host_store(Some(&sink)).cloned()
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

    fn emit(&self, level: LogLevel, message: String) {
        if let Err(error) = self.telemetry(level, message) {
            Self::report_error(error);
        }
    }

    fn report_error(error: LoggingError) {
        eprintln!("marix logger failed: {error}");
    }

    fn telemetry(&self, level: LogLevel, message: String) -> Result<(), LoggingError> {
        let Some(source) = self.source.get().copied() else {
            return Ok(());
        };
        let mut message = LogMessage::new(level, message);
        message.source = source;
        message.session_id = *self
            .session_id
            .read()
            .unwrap_or_else(|error| error.into_inner());
        let Some(sink) = self.sink.get() else {
            return Ok(());
        };

        let host = {
            let mut sink = sink
                .lock()
                .map_err(|error| LoggingError::Io(error.to_string()))?;
            let mut replacement = None;
            let host = match &mut *sink {
                Sink::Host(host) => Some(host.clone()),
                Sink::File(file) => {
                    file.append(&message)?;
                    None
                }
                Sink::Remote {
                    sender,
                    fallback_path,
                } => {
                    if sender.try_send(message.clone()).is_err() {
                        let mut file = LogFile::create(fallback_path)?;
                        file.append(&message)?;
                        replacement = Some(Sink::File(file));
                    }
                    None
                }
            };
            if let Some(replacement) = replacement {
                *sink = replacement;
            }
            host
        };
        if let Some(host) = host {
            message.stamp_arrival();
            host.record(message)?;
        }
        Ok(())
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

    fn remote_sink(
        fallback_path: PathBuf,
        connect_sender: impl FnOnce() -> Result<NetSender<LogMessage>, LoggingError>,
    ) -> Result<Sink, LoggingError> {
        let sender = connect_sender()?;
        Ok(Sink::Remote {
            sender,
            fallback_path,
        })
    }

    fn record(&self, mut message: LogMessage) -> Result<(), LoggingError> {
        message.stamp_arrival();
        Self::host_sink()?.record(message)
    }
}

enum Sink {
    Host(HostStore),
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

fn host_store(sink: Option<&Sink>) -> Result<&HostStore, LoggingError> {
    match sink {
        Some(Sink::Host(store)) => Ok(store),
        _ => Err(LoggingError::NotHosting),
    }
}

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
    let runtime = match crate::external::tokio::Builder::new_current_thread()
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
