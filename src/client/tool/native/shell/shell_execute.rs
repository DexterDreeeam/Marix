use std::{
    io::{BufRead, BufReader},
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use crate::client::tool::{
    Tool, ToolCategory, ToolError, ToolExecutionResult, ToolInvocation, ToolInvocationStatus,
    ToolPreview, ToolRuntime, ToolType,
};
use crate::common::config::Platform;
use crate::common::external::*;

pub struct ShellExecuteTool;

impl ShellExecuteTool {
    pub const PREVIEW: ToolPreview = ToolPreview {
        name: "native_shell_execute",
        description: "Run a native command through the current operating system shell.",
        schema: r#"{"type":"object","properties":{"command":{"type":"string"},"cwd":{"type":"string"},"timeout_ms":{"type":"integer","minimum":1}},"required":["command"],"additionalProperties":false}"#,
    };
}

impl Tool for ShellExecuteTool {
    fn tool_type(&self) -> ToolType {
        ToolType::Primary
    }

    fn platform(&self) -> Platform {
        Platform::All
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Shell
    }

    fn name(&self) -> &'static str {
        Self::PREVIEW.name
    }

    fn description(&self) -> &'static str {
        Self::PREVIEW.description
    }

    fn schema(&self) -> &'static str {
        Self::PREVIEW.schema
    }

    fn invoke(&self, invocation: ToolInvocation) -> Result<ToolRuntime, ToolError> {
        let arguments = parse_arguments(&invocation.arguments)?;
        let (status_tx, status_rx) = mpsc::channel();
        let (cancel_tx, cancel_rx) = mpsc::channel();

        thread::spawn(move || {
            let _ = status_tx.send(ToolInvocationStatus::Started);
            match execute_shell(arguments, &status_tx, &cancel_rx) {
                Ok(ToolExecutionResult::Complete(())) => {
                    let _ = status_tx.send(ToolInvocationStatus::Complete);
                }
                Ok(ToolExecutionResult::Cancelled) => {
                    let _ = status_tx.send(ToolInvocationStatus::Cancelled);
                }
                Err(error) => {
                    let _ = status_tx.send(ToolInvocationStatus::Failed(error));
                }
            }
        });

        Ok(ToolRuntime::new(status_rx, cancel_tx))
    }
}

// -- Private -- //

struct ShellExecuteArguments {
    command: String,
    cwd: Option<String>,
    timeout_ms: Option<u64>,
}

fn parse_arguments(arguments: &str) -> Result<ShellExecuteArguments, ToolError> {
    let value: serde_json::Value = serde_json::from_str(arguments)
        .map_err(|error| ToolError::InvalidArguments(error.to_string()))?;
    let command = value
        .get("command")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| ToolError::InvalidArguments("missing string field: command".to_string()))?;
    let cwd = value
        .get("cwd")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let timeout_ms = value.get("timeout_ms").and_then(serde_json::Value::as_u64);

    Ok(ShellExecuteArguments {
        command,
        cwd,
        timeout_ms,
    })
}

fn execute_shell(
    arguments: ShellExecuteArguments,
    status_tx: &mpsc::Sender<ToolInvocationStatus>,
    cancel_rx: &mpsc::Receiver<()>,
) -> Result<ToolExecutionResult<()>, ToolError> {
    let mut command = shell_command(&arguments.command);
    if let Some(cwd) = arguments.cwd {
        command.current_dir(cwd);
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|error| ToolError::ExecutionFailed(error.to_string()))?;
    if let Some(stdout) = child.stdout.take() {
        spawn_stream_reader("stdout", stdout, status_tx.clone());
    }
    if let Some(stderr) = child.stderr.take() {
        spawn_stream_reader("stderr", stderr, status_tx.clone());
    }

    let deadline = arguments
        .timeout_ms
        .map(|timeout_ms| Instant::now() + Duration::from_millis(timeout_ms));
    loop {
        if cancel_rx.try_recv().is_ok() {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(ToolExecutionResult::Cancelled);
        }
        if let Some(status) = child
            .try_wait()
            .map_err(|error| ToolError::ExecutionFailed(error.to_string()))?
        {
            let _ = status_tx.send(ToolInvocationStatus::Running(
                self::serde_json::json!({ "status": status.code() }).to_string(),
            ));
            return Ok(ToolExecutionResult::Complete(()));
        }
        if let Some(deadline) = deadline {
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ToolError::ExecutionFailed(format!(
                    "command timed out after {} ms",
                    arguments.timeout_ms.unwrap_or_default()
                )));
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn spawn_stream_reader<R>(
    stream: &'static str,
    reader: R,
    status_tx: mpsc::Sender<ToolInvocationStatus>,
) where
    R: std::io::Read + Send + 'static,
{
    thread::spawn(move || {
        for line in BufReader::new(reader).lines() {
            match line {
                Ok(line) => {
                    let _ = status_tx.send(ToolInvocationStatus::Running(
                        self::serde_json::json!({
                            "stream": stream,
                            "content": line
                        })
                        .to_string(),
                    ));
                }
                Err(error) => {
                    let _ = status_tx.send(ToolInvocationStatus::Failed(
                        ToolError::ExecutionFailed(error.to_string()),
                    ));
                    break;
                }
            }
        }
    });
}

fn shell_command(command: &str) -> Command {
    if cfg!(windows) {
        let mut shell = Command::new("cmd");
        shell.args(["/C", command]);
        shell
    } else {
        let mut shell = Command::new("sh");
        shell.args(["-c", command]);
        shell
    }
}
