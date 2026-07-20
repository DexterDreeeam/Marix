use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

pub(super) struct Output {
    pub(super) content: String,
    pub(super) next_offset: u64,
    pub(super) truncated: bool,
}

pub(super) fn read(
    stdout_path: &Path,
    stdout_offset: u64,
    stderr_path: &Path,
    stderr_offset: u64,
    max_bytes: u64,
) -> Result<(Output, Output), String> {
    let stdout = OutputSnapshot::from_path(stdout_path, stdout_offset)?;
    let stderr = OutputSnapshot::from_path(stderr_path, stderr_offset)?;
    let (stdout_budget, stderr_budget) = output_budgets(&stdout, &stderr, max_bytes);
    Ok((
        read_file(stdout_path, stdout, stdout_budget)?,
        read_file(stderr_path, stderr, stderr_budget)?,
    ))
}

// -- Private -- //

struct OutputSnapshot {
    start: u64,
    available: u64,
}

impl OutputSnapshot {
    fn from_path(path: &Path, offset: u64) -> Result<Self, String> {
        reject_symlink(path)?;
        let length = fs::metadata(path)
            .map_err(|error| format!("failed to inspect process output: {error}"))?
            .len();
        if offset > length {
            return Err("output offset is beyond the captured output".to_owned());
        }
        Ok(Self {
            start: offset,
            available: length - offset,
        })
    }
}

fn output_budgets(stdout: &OutputSnapshot, stderr: &OutputSnapshot, max_bytes: u64) -> (u64, u64) {
    let first_stdout_budget = stdout.available.min(max_bytes.div_ceil(2));
    let stderr_budget = stderr
        .available
        .min(max_bytes.saturating_sub(first_stdout_budget));
    let remaining = max_bytes
        .saturating_sub(first_stdout_budget)
        .saturating_sub(stderr_budget);
    let stdout_budget = first_stdout_budget.saturating_add(
        stdout
            .available
            .saturating_sub(first_stdout_budget)
            .min(remaining),
    );
    (stdout_budget, stderr_budget)
}

fn read_file(path: &Path, snapshot: OutputSnapshot, max_bytes: u64) -> Result<Output, String> {
    let byte_count = snapshot.available.min(max_bytes);
    let mut file =
        File::open(path).map_err(|error| format!("failed to read process output: {error}"))?;
    file.seek(SeekFrom::Start(snapshot.start))
        .map_err(|error| format!("failed to read process output: {error}"))?;
    let mut bytes = vec![0; byte_count as usize];
    let mut bytes_read = 0;
    while bytes_read < bytes.len() {
        let count = file
            .read(&mut bytes[bytes_read..])
            .map_err(|error| format!("failed to read process output: {error}"))?;
        if count == 0 {
            break;
        }
        bytes_read += count;
    }
    bytes.truncate(bytes_read);
    Ok(Output {
        content: String::from_utf8_lossy(&bytes).into_owned(),
        next_offset: snapshot.start + bytes_read as u64,
        truncated: snapshot.available > bytes_read as u64,
    })
}

fn reject_symlink(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("failed to inspect process output: {error}"))?;
    if metadata.file_type().is_symlink() {
        return Err("process output must not be a symbolic link".to_owned());
    }
    Ok(())
}
