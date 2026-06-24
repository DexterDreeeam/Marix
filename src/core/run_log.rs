use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

static RUN_LOG: OnceLock<Mutex<File>> = OnceLock::new();

pub fn init() -> std::io::Result<PathBuf> {
    let log_dir = std::env::var_os("MARIX_CORE_LOG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(".target").join("run-logs"));
    std::fs::create_dir_all(&log_dir)?;
    let path = log_dir.join(format!(
        "marix-core-{}-{}.log",
        timestamp_seconds(),
        std::process::id()
    ));
    let file = OpenOptions::new()
        .create_new(true)
        .append(true)
        .open(&path)?;
    let _ = RUN_LOG.set(Mutex::new(file));
    record(format!("core log initialized: {}", path.display()));
    Ok(path)
}

pub fn record(message: impl AsRef<str>) {
    let Some(log) = RUN_LOG.get() else {
        return;
    };
    let timestamp = timestamp_seconds();
    match log.lock() {
        Ok(mut file) => {
            if let Err(error) = writeln!(file, "[{timestamp}] {}", message.as_ref()) {
                eprintln!("core log write failed: {error}");
            }
        }
        Err(error) => eprintln!("core log lock failed: {error}"),
    }
}

fn timestamp_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}
