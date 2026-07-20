use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use marix_common::external::{
    serde_json::{Value, from_str, json, to_string},
    uuid::Uuid,
};

use super::output;

const LOCK_RETRIES: u32 = 200;
const LOCK_RETRY_DELAY: Duration = Duration::from_millis(10);
const MAX_OUTPUT_BYTES: u64 = 65_536;
const RETENTION_AGE: Duration = Duration::from_secs(24 * 60 * 60);

pub(super) fn start(command: &str, args: &[String], cwd: Option<&str>) -> Result<String, String> {
    if command.trim().is_empty() {
        return Err("command must not be empty".to_owned());
    }
    if command.len() > 32 * 1024 {
        return Err("command exceeds the 32 KiB limit".to_owned());
    }

    let lock = RegistryLock::acquire()?;
    let root = lock.root();
    cleanup_stopped_entries(root)?;

    let id = Uuid::new_v4().to_string();
    let stdout_path = output_path(root, &id, "stdout")?;
    let stderr_path = output_path(root, &id, "stderr")?;
    let stdout = create_output_file(&stdout_path)?;
    let stderr = create_output_file(&stderr_path)?;
    let mut process = Command::new(command);
    process
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    if let Some(cwd) = cwd {
        process.current_dir(cwd);
    }
    let mut child = process
        .spawn()
        .map_err(|error| format!("failed to start process: {error}"))?;
    let creation_date = match process_creation_date(child.id()) {
        Ok(creation_date) => creation_date,
        Err(error) => {
            let _ = child.kill();
            let _ = fs::remove_file(stdout_path);
            let _ = fs::remove_file(stderr_path);
            return Err(error);
        }
    };
    let record = ProcessRecord {
        id: id.clone(),
        pid: child.id(),
        started_at_secs: now_secs()?,
        creation_date,
        stopped: false,
        stopped_at_secs: None,
    };
    if let Err(error) = write_record(root, &record) {
        let _ = child.kill();
        let _ = fs::remove_file(stdout_path);
        let _ = fs::remove_file(stderr_path);
        return Err(error);
    }

    to_string(&json!({
        "process_id": id,
        "pid": record.pid,
    }))
    .map_err(|error| format!("failed to serialize process result: {error}"))
}

pub(super) fn read_output(
    process_id: &str,
    stdout_offset: u64,
    stderr_offset: u64,
    max_bytes: u64,
) -> Result<String, String> {
    if !(1..=MAX_OUTPUT_BYTES).contains(&max_bytes) {
        return Err("max_bytes must be an integer from 1 through 65536".to_owned());
    }
    let lock = RegistryLock::acquire()?;
    let root = lock.root();
    let mut record = read_record(root, process_id)?;
    let running = process_state(&record)?;
    if !running {
        record.mark_stopped()?;
        write_record(root, &record)?;
    }

    let stdout_path = output_path(root, &record.id, "stdout")?;
    let stderr_path = output_path(root, &record.id, "stderr")?;
    let (stdout, stderr) = output::read(
        &stdout_path,
        stdout_offset,
        &stderr_path,
        stderr_offset,
        max_bytes,
    )?;
    let output::Output {
        content: stdout,
        next_offset: stdout_next_offset,
        truncated: stdout_truncated,
    } = stdout;
    let output::Output {
        content: stderr,
        next_offset: stderr_next_offset,
        truncated: stderr_truncated,
    } = stderr;

    to_string(&json!({
        "process_id": record.id,
        "pid": record.pid,
        "running": running,
        "stdout": stdout,
        "stderr": stderr,
        "next_offset": {
            "stdout": stdout_next_offset,
            "stderr": stderr_next_offset,
        },
        "truncated": stdout_truncated || stderr_truncated,
    }))
    .map_err(|error| format!("failed to serialize process output: {error}"))
}

pub(super) fn stop(process_id: &str) -> Result<String, String> {
    let lock = RegistryLock::acquire()?;
    let root = lock.root();
    let mut record = read_record(root, process_id)?;
    if record.stopped {
        return to_string(&json!({
            "process_id": record.id,
            "stopped": true,
            "already_stopped": true,
        }))
        .map_err(|error| format!("failed to serialize process result: {error}"));
    }

    if !process_state(&record)? {
        record.mark_stopped()?;
        write_record(root, &record)?;
        return to_string(&json!({
            "process_id": record.id,
            "stopped": false,
            "already_exited": true,
        }))
        .map_err(|error| format!("failed to serialize process result: {error}"));
    }

    let output = Command::new(taskkill_program()?)
        .args(["/PID", &record.pid.to_string(), "/T", "/F"])
        .output()
        .map_err(|error| format!("failed to stop process: {error}"))?;
    if !output.status.success() && process_state(&record)? {
        return Err(format!(
            "taskkill failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    record.mark_stopped()?;
    write_record(root, &record)?;
    to_string(&json!({
        "process_id": record.id,
        "stopped": true,
        "already_stopped": false,
    }))
    .map_err(|error| format!("failed to serialize process result: {error}"))
}

// -- Private -- //

struct ProcessRecord {
    id: String,
    pid: u32,
    started_at_secs: u64,
    creation_date: Option<String>,
    stopped: bool,
    stopped_at_secs: Option<u64>,
}

impl ProcessRecord {
    fn mark_stopped(&mut self) -> Result<(), String> {
        self.stopped = true;
        if self.stopped_at_secs.is_none() {
            self.stopped_at_secs = Some(now_secs()?);
        }
        Ok(())
    }
}

struct RegistryLock {
    root: PathBuf,
    file: File,
}

impl RegistryLock {
    fn acquire() -> Result<Self, String> {
        let root = registry_root();
        fs::create_dir_all(&root)
            .map_err(|error| format!("failed to create process registry: {error}"))?;
        reject_symlink(&root, "process registry")?;
        let path = root.join("registry.lock");
        reject_symlink_if_exists(&path, "process registry lock")?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&path)
            .map_err(|error| format!("failed to open process registry lock: {error}"))?;

        for _ in 0..LOCK_RETRIES {
            match file.try_lock() {
                Ok(()) => return Ok(Self { root, file }),
                Err(std::fs::TryLockError::WouldBlock) => {
                    thread::sleep(LOCK_RETRY_DELAY);
                }
                Err(std::fs::TryLockError::Error(error)) => {
                    return Err(format!("failed to lock process registry: {error}"));
                }
            }
        }

        Err("timed out waiting for the process registry".to_owned())
    }

    fn root(&self) -> &Path {
        &self.root
    }
}

impl Drop for RegistryLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

fn registry_root() -> PathBuf {
    std::env::temp_dir().join("marix-processes")
}

fn cleanup_stopped_entries(root: &Path) -> Result<(), String> {
    let cutoff = now_secs()?.saturating_sub(RETENTION_AGE.as_secs());
    let entries =
        fs::read_dir(root).map_err(|error| format!("failed to read process registry: {error}"))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("failed to read process registry: {error}"))?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let Some(id) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        if !valid_process_id(id) {
            continue;
        }
        let mut record = match read_record(root, id) {
            Ok(record) => record,
            Err(_) => continue,
        };
        if !record.stopped {
            match process_state(&record) {
                Ok(true) => continue,
                Ok(false) => {
                    if record.mark_stopped().is_err() || write_record(root, &record).is_err() {
                        continue;
                    }
                }
                Err(_) => continue,
            }
        }
        if record.stopped_at_secs.is_none() {
            if record.mark_stopped().is_err() || write_record(root, &record).is_err() {
                continue;
            }
        }
        let Some(stopped_at_secs) = record.stopped_at_secs else {
            continue;
        };
        if stopped_at_secs < cutoff {
            let _ = fs::remove_file(record_path(root, id)?);
            let _ = fs::remove_file(output_path(root, id, "stdout")?);
            let _ = fs::remove_file(output_path(root, id, "stderr")?);
        }
    }
    Ok(())
}

fn read_record(root: &Path, process_id: &str) -> Result<ProcessRecord, String> {
    if !valid_process_id(process_id) {
        return Err("process_id must be a UUID issued by start_process".to_owned());
    }
    let path = record_path(root, process_id)?;
    reject_symlink(&path, "process registry record")?;
    let content = fs::read_to_string(&path)
        .map_err(|error| format!("unknown process_id '{process_id}': {error}"))?;
    let value: Value =
        from_str(&content).map_err(|error| format!("invalid process registry record: {error}"))?;
    let Some(id) = value.get("id").and_then(Value::as_str) else {
        return Err("invalid process registry record".to_owned());
    };
    let Some(pid) = value
        .get("pid")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
    else {
        return Err("invalid process registry record".to_owned());
    };
    let Some(started_at_secs) = value.get("started_at_secs").and_then(Value::as_u64) else {
        return Err("invalid process registry record".to_owned());
    };
    let creation_date = match value.get("creation_date") {
        None | Some(Value::Null) => None,
        Some(value) => Some(
            value
                .as_str()
                .ok_or_else(|| "invalid process registry record".to_owned())?
                .to_owned(),
        ),
    };
    let Some(stopped) = value.get("stopped").and_then(Value::as_bool) else {
        return Err("invalid process registry record".to_owned());
    };
    let stopped_at_secs = match value.get("stopped_at_secs") {
        None | Some(Value::Null) => None,
        Some(value) => Some(
            value
                .as_u64()
                .ok_or_else(|| "invalid process registry record".to_owned())?,
        ),
    };
    if !stopped && stopped_at_secs.is_some() {
        return Err("invalid process registry record".to_owned());
    }
    let record = ProcessRecord {
        id: id.to_owned(),
        pid,
        started_at_secs,
        creation_date,
        stopped,
        stopped_at_secs,
    };
    if record.id != process_id || record.pid == 0 {
        return Err("invalid process registry record".to_owned());
    }
    Ok(record)
}

fn write_record(root: &Path, record: &ProcessRecord) -> Result<(), String> {
    let path = record_path(root, &record.id)?;
    reject_symlink_if_exists(&path, "process registry record")?;
    let content = to_string(&json!({
        "id": record.id,
        "pid": record.pid,
        "started_at_secs": record.started_at_secs,
        "creation_date": record.creation_date,
        "stopped": record.stopped,
        "stopped_at_secs": record.stopped_at_secs,
    }))
    .map_err(|error| format!("failed to serialize process registry record: {error}"))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .map_err(|error| format!("failed to write process registry record: {error}"))?;
    file.write_all(content.as_bytes())
        .map_err(|error| format!("failed to write process registry record: {error}"))
}

fn record_path(root: &Path, process_id: &str) -> Result<PathBuf, String> {
    if !valid_process_id(process_id) {
        return Err("process_id must be a UUID issued by start_process".to_owned());
    }
    Ok(root.join(format!("{process_id}.json")))
}

fn output_path(root: &Path, process_id: &str, stream: &str) -> Result<PathBuf, String> {
    if !valid_process_id(process_id) || !matches!(stream, "stdout" | "stderr") {
        return Err("invalid process registry path".to_owned());
    }
    Ok(root.join(format!("{process_id}.{stream}")))
}

fn valid_process_id(process_id: &str) -> bool {
    Uuid::parse_str(process_id)
        .map(|uuid| uuid.hyphenated().to_string() == process_id)
        .unwrap_or(false)
}

fn create_output_file(path: &Path) -> Result<File, String> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("failed to create process output file: {error}"))
}

fn process_state(record: &ProcessRecord) -> Result<bool, String> {
    if record.stopped {
        return Ok(false);
    }
    let Some(expected) = record.creation_date.as_deref() else {
        return Ok(false);
    };
    match process_creation_date(record.pid)? {
        Some(actual) if actual == expected => Ok(true),
        Some(_) => Err(
            "process_id no longer identifies the process started by Marix; \
             refusing to inspect or stop it"
                .to_owned(),
        ),
        None => Ok(false),
    }
}

fn process_creation_date(pid: u32) -> Result<Option<String>, String> {
    let script = format!(
        "$process = Get-CimInstance -ClassName Win32_Process -Filter \
         'ProcessId = {pid}' -ErrorAction SilentlyContinue; \
         if ($null -eq $process) {{ 'missing' }} \
         else {{ $process.CreationDate }}"
    );
    let output = Command::new(powershell_program()?)
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .map_err(|error| format!("failed to validate process ownership: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "failed to validate process ownership: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    match String::from_utf8_lossy(&output.stdout).trim() {
        "" | "missing" => Ok(None),
        creation_date => Ok(Some(creation_date.to_owned())),
    }
}

fn powershell_program() -> Result<PathBuf, String> {
    Ok(system_program("WindowsPowerShell\\v1.0\\powershell.exe")?)
}

fn taskkill_program() -> Result<PathBuf, String> {
    system_program("taskkill.exe")
}

fn system_program(program: &str) -> Result<PathBuf, String> {
    let root = env::var_os("SystemRoot").ok_or_else(|| "SystemRoot is unavailable".to_owned())?;
    let path = PathBuf::from(root).join("System32").join(program);
    if !path.is_absolute() {
        return Err("SystemRoot does not contain an absolute path".to_owned());
    }
    Ok(path)
}

fn reject_symlink(path: &Path, label: &str) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("failed to inspect {label}: {error}"))?;
    if metadata.file_type().is_symlink() {
        return Err(format!("{label} must not be a symbolic link"));
    }
    Ok(())
}

fn reject_symlink_if_exists(path: &Path, label: &str) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(format!("{label} must not be a symbolic link"))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("failed to inspect {label}: {error}")),
    }
}

fn now_secs() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|error| format!("system clock is before the Unix epoch: {error}"))
}
