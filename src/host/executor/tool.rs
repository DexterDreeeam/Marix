use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use marix_common::external::*;
use marix_protocol::ToolPreview;

#[derive(Clone)]
pub struct Tool {
    path: PathBuf,
    preview: ToolPreview,
}

impl Tool {
    pub fn load(path: &Path) -> Option<Self> {
        let output = Command::new(path).arg("--preview").output().ok()?;
        if !output.status.success() {
            return None;
        }
        let stdout = String::from_utf8(output.stdout).ok()?;
        let preview = ToolPreview::from_json(stdout.trim()).ok()?;
        Some(Self {
            path: path.to_path_buf(),
            preview,
        })
    }

    pub fn name(&self) -> String {
        self.preview.name.clone()
    }

    pub fn description(&self) -> String {
        self.preview.description.clone()
    }

    pub fn preview(&self) -> ToolPreview {
        self.preview.clone()
    }

    pub fn execute(&self, input: &str) -> String {
        let mut child = match Command::new(&self.path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(error) => {
                return Self::failure(format!(
                    "failed to spawn tool {}: {error}",
                    self.path.display()
                ));
            }
        };
        let write_error = match child.stdin.take() {
            Some(mut stdin) => {
                let error = stdin.write_all(input.as_bytes()).err().map(|error| {
                    format!(
                        "failed to write input to tool {}: {error}",
                        self.path.display()
                    )
                });
                drop(stdin);
                error
            }
            None => Some(format!(
                "failed to open stdin for tool {}",
                self.path.display()
            )),
        };
        let output = match child.wait_with_output() {
            Ok(output) => output,
            Err(error) => {
                return Self::failure(format!(
                    "failed to wait for tool {}: {error}",
                    self.path.display()
                ));
            }
        };
        if !output.status.success() {
            return serde_json::json!({
                "error": "tool process exited unsuccessfully",
                "exit_code": output.status.code(),
                "stdout": String::from_utf8_lossy(&output.stdout),
                "stderr": String::from_utf8_lossy(&output.stderr),
                "stdin_error": write_error.as_deref(),
            })
            .to_string();
        }
        if let Some(error) = write_error {
            return Self::failure(error);
        }
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }
}

// -- Private -- //

impl Tool {
    fn failure(message: String) -> String {
        serde_json::json!({ "error": message }).to_string()
    }
}
