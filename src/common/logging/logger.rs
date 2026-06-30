use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::common::config::{Config, LoggingConfig};
use crate::common::logging::{LogMessage, LogTag, LoggingError};

/// Writes an info, warning, or error log message when the matching config flag is enabled.
pub fn log(message: impl Into<LogMessage>) -> Result<(), LoggingError> {
    let message = message.into();
    if !is_log_enabled(message.tag)? {
        return Ok(());
    }
    write_log_line(message.tag.label(), &message.message)
}

/// Writes a debug-only log message; the closure is evaluated only in debug builds.
#[cfg(debug_assertions)]
pub fn debug<T>(message: impl FnOnce() -> T) -> Result<(), LoggingError>
where
    T: Into<String>,
{
    write_log_line("DEBUG", &message().into())
}

/// Writes a debug-only log message; release builds keep the API and skip the closure.
#[cfg(not(debug_assertions))]
pub fn debug<T>(_message: impl FnOnce() -> T) -> Result<(), LoggingError>
where
    T: Into<String>,
{
    Ok(())
}

// -- Private -- //

struct LogFile {
    file: File,
}

impl LogFile {
    fn create() -> Result<Self, LoggingError> {
        let directory = log_directory()?;
        std::fs::create_dir_all(&directory)?;
        prune_log_files(&directory)?;
        let path = next_log_path(&directory)?;
        let file = OpenOptions::new()
            .append(true)
            .create_new(true)
            .open(path)?;
        Ok(Self { file })
    }

    fn write_line(&mut self, tag: &str, message: &str) -> Result<(), LoggingError> {
        let timestamp = timestamp_millis()?;
        writeln!(
            self.file,
            "{timestamp} [{tag}] {}",
            normalize_log_message(message)
        )?;
        self.file.flush()?;
        self.file.sync_data()?;
        Ok(())
    }
}

static LOGGING_CONFIG: OnceLock<Result<LoggingConfig, LoggingError>> = OnceLock::new();
static LOG_FILE: OnceLock<Result<Mutex<LogFile>, LoggingError>> = OnceLock::new();

const MAX_LOG_FILE_COUNT: usize = 20;

impl LogTag {
    fn label(self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Warning => "WARNING",
            Self::Error => "ERROR",
        }
    }
}

fn is_log_enabled(tag: LogTag) -> Result<bool, LoggingError> {
    let config = logging_config()?;
    Ok(match tag {
        LogTag::Info => config.enable_log_info,
        LogTag::Warning => config.enable_log_warning,
        LogTag::Error => config.enable_log_error,
    })
}

fn logging_config() -> Result<&'static LoggingConfig, LoggingError> {
    match LOGGING_CONFIG.get_or_init(load_logging_config) {
        Ok(config) => Ok(config),
        Err(error) => Err(error.clone()),
    }
}

fn load_logging_config() -> Result<LoggingConfig, LoggingError> {
    Config::load()
        .map(|config| config.logging)
        .map_err(LoggingError::Config)
}

fn write_log_line(tag: &str, message: &str) -> Result<(), LoggingError> {
    let file = log_file()?;
    let mut guard = file
        .lock()
        .map_err(|error| LoggingError::Io(error.to_string()))?;
    guard.write_line(tag, message)
}

fn log_file() -> Result<&'static Mutex<LogFile>, LoggingError> {
    match LOG_FILE.get_or_init(|| LogFile::create().map(Mutex::new)) {
        Ok(file) => Ok(file),
        Err(error) => Err(error.clone()),
    }
}

#[cfg(windows)]
fn log_directory() -> Result<PathBuf, LoggingError> {
    Ok(PathBuf::from(r"C:\marix"))
}

#[cfg(not(windows))]
fn log_directory() -> Result<PathBuf, LoggingError> {
    Ok(PathBuf::from(&logging_config()?.directory))
}

fn prune_log_files(directory: &Path) -> Result<(), LoggingError> {
    let mut entries = log_file_entries(directory)?;
    entries.sort_by(|left, right| {
        left.modified
            .cmp(&right.modified)
            .then_with(|| left.path.cmp(&right.path))
    });

    let remove_count = entries.len().saturating_sub(MAX_LOG_FILE_COUNT - 1);
    for entry in entries.into_iter().take(remove_count) {
        std::fs::remove_file(entry.path)?;
    }
    Ok(())
}

fn log_file_entries(directory: &Path) -> Result<Vec<LogFileEntry>, LoggingError> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file()
            || path.extension().and_then(|extension| extension.to_str()) != Some("log")
        {
            continue;
        }
        let modified = entry.metadata()?.modified().unwrap_or(UNIX_EPOCH);
        entries.push(LogFileEntry { path, modified });
    }
    Ok(entries)
}

fn next_log_path(directory: &Path) -> Result<PathBuf, LoggingError> {
    let timestamp = timestamp_millis()?;
    for suffix in 0..=999 {
        let file_name = if suffix == 0 {
            format!("{timestamp}.log")
        } else {
            format!("{timestamp}-{suffix}.log")
        };
        let path = directory.join(file_name);
        if !path.exists() {
            return Ok(path);
        }
    }
    Err(LoggingError::Io(
        "could not allocate unique log file name".to_owned(),
    ))
}

fn timestamp_millis() -> Result<u128, LoggingError> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())
}

fn normalize_log_message(message: &str) -> String {
    message.replace("\r\n", "\\n").replace(['\r', '\n'], "\\n")
}

struct LogFileEntry {
    path: PathBuf,
    modified: SystemTime,
}
