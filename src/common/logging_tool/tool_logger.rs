use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct ToolLogger {
    file: File,
}

impl ToolLogger {
    pub fn new() -> std::io::Result<Self> {
        let executable = std::env::current_exe()?;
        let directory = executable.parent().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "current executable has no parent directory",
            )
        })?;
        let stem = executable.file_stem().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "current executable has no file stem",
            )
        })?;

        for _ in 0..16 {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(io::Error::other)?
                .as_nanos();
            let mut file_name = OsString::from(stem);
            file_name.push(format!("_{timestamp}.log"));

            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(directory.join(file_name))
            {
                Ok(file) => return Ok(Self { file }),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }

        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not create a unique tool log file",
        ))
    }

    pub fn log(&mut self, message: &str) -> std::io::Result<()> {
        self.file.write_all(message.as_bytes())?;
        self.file.write_all(b"\n")?;
        self.file.flush()
    }
}
