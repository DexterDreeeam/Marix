use std::{
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::Duration,
};

use crate::client::tool::{
    Tool, ToolCategory, ToolError, ToolExecutionResult, ToolInvocation, ToolInvocationStatus,
    ToolPreview, ToolRuntime, ToolType,
};
use crate::common::config::Platform;
use crate::common::external::*;

pub struct ProcessListTool;

impl ProcessListTool {
    pub const PREVIEW: ToolPreview = ToolPreview {
        name: "native_process_list",
        description: "List currently running local processes.",
        schema: r#"{"type":"object","properties":{},"additionalProperties":false}"#,
    };
}

impl Tool for ProcessListTool {
    fn tool_type(&self) -> ToolType {
        ToolType::Native
    }

    fn platform(&self) -> Platform {
        Platform::Win
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Process
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

    fn invoke(&self, _invocation: ToolInvocation) -> Result<ToolRuntime, ToolError> {
        if !cfg!(windows) {
            return Err(ToolError::Denied(
                "native_process_list is currently only supported on Windows".to_string(),
            ));
        }

        let (status_tx, status_rx) = mpsc::channel();
        let (cancel_tx, cancel_rx) = mpsc::channel();
        thread::spawn(move || {
            let _ = status_tx.send(ToolInvocationStatus::Started);
            if cancel_rx.try_recv().is_ok() {
                let _ = status_tx.send(ToolInvocationStatus::Cancelled);
                return;
            }
            match run_tasklist(&cancel_rx) {
                Ok(ToolExecutionResult::Complete(processes)) => {
                    for process in processes {
                        if cancel_rx.try_recv().is_ok() {
                            let _ = status_tx.send(ToolInvocationStatus::Cancelled);
                            return;
                        }
                        let _ = status_tx.send(ToolInvocationStatus::Running(process));
                    }
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

fn run_tasklist(
    cancel_rx: &mpsc::Receiver<()>,
) -> Result<ToolExecutionResult<Vec<String>>, ToolError> {
    let mut child = Command::new("tasklist")
        .args(["/FO", "CSV", "/NH"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| ToolError::ExecutionFailed(error.to_string()))?;

    loop {
        if cancel_rx.try_recv().is_ok() {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(ToolExecutionResult::Cancelled);
        }
        if child
            .try_wait()
            .map_err(|error| ToolError::ExecutionFailed(error.to_string()))?
            .is_some()
        {
            let output = child
                .wait_with_output()
                .map_err(|error| ToolError::ExecutionFailed(error.to_string()))?;
            if !output.status.success() {
                return Err(ToolError::ExecutionFailed(
                    String::from_utf8_lossy(&output.stderr).to_string(),
                ));
            }

            return Ok(ToolExecutionResult::Complete(
                String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .filter_map(parse_tasklist_line)
                    .collect(),
            ));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn parse_tasklist_line(line: &str) -> Option<String> {
    let fields = parse_csv_record(line);
    if fields.len() < 5 {
        return None;
    }

    Some(
        self::serde_json::json!({
            "name": fields[0],
            "pid": fields[1].parse::<u32>().ok(),
            "session_name": fields[2],
            "session_number": fields[3].parse::<u32>().ok(),
            "memory_usage": fields[4]
        })
        .to_string(),
    )
}

fn parse_csv_record(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut chars = line.chars().peekable();
    let mut in_quotes = false;

    while let Some(character) = chars.next() {
        match character {
            '"' if in_quotes && chars.peek() == Some(&'"') => {
                field.push('"');
                let _ = chars.next();
            }
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                fields.push(field);
                field = String::new();
            }
            _ => field.push(character),
        }
    }
    fields.push(field);

    fields
}
