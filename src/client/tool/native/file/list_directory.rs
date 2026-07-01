use std::{fs, path::Path, sync::mpsc, thread};

use crate::client::tool::{
    Tool, ToolCategory, ToolError, ToolExecutionResult, ToolInvocation, ToolInvocationStatus,
    ToolSchema, ToolRuntime, ToolType,
};
use crate::common::config::Platform;
use crate::common::external::*;

pub struct ListDirectoryTool;

impl ListDirectoryTool {
    pub const NAME: &'static str = "native_list_directory";
    pub const DESCRIPTION: &'static str = "List entries under a local directory.";
    pub const SCHEMA: &'static str = r#"{"type":"object","properties":{"path":{"type":"string"},"recursive":{"type":"boolean"}},"required":["path"],"additionalProperties":false}"#;
}

impl Tool for ListDirectoryTool {
    fn tool_type(&self) -> ToolType {
        ToolType::Native
    }

    fn platform(&self) -> Platform {
        Platform::All
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::File
    }

    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn description(&self) -> &'static str {
        Self::DESCRIPTION
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema::new(Self::SCHEMA)
    }

    fn invoke(&self, invocation: ToolInvocation) -> Result<ToolRuntime, ToolError> {
        let arguments = parse_arguments(&invocation.parameter.payload)?;
        let (status_tx, status_rx) = mpsc::channel();
        let (cancel_tx, cancel_rx) = mpsc::channel();

        thread::spawn(move || {
            let _ = status_tx.send(ToolInvocationStatus::Started);
            let result = collect_entries(
                Path::new(&arguments.path),
                arguments.recursive,
                &status_tx,
                &cancel_rx,
            );
            match result {
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

struct ListDirectoryArguments {
    path: String,
    recursive: bool,
}

fn parse_arguments(arguments: &str) -> Result<ListDirectoryArguments, ToolError> {
    let value: serde_json::Value = serde_json::from_str(arguments)
        .map_err(|error| ToolError::InvalidArguments(error.to_string()))?;
    let path = value
        .get("path")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| ToolError::InvalidArguments("missing string field: path".to_string()))?;
    let recursive = value
        .get("recursive")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    Ok(ListDirectoryArguments { path, recursive })
}

fn collect_entries(
    path: &Path,
    recursive: bool,
    status_tx: &mpsc::Sender<ToolInvocationStatus>,
    cancel_rx: &mpsc::Receiver<()>,
) -> Result<ToolExecutionResult<()>, ToolError> {
    if cancel_rx.try_recv().is_ok() {
        return Ok(ToolExecutionResult::Cancelled);
    }

    let read_dir = fs::read_dir(path).map_err(|error| {
        ToolError::ExecutionFailed(format!("failed to list {}: {error}", path.display()))
    })?;

    for entry in read_dir {
        if cancel_rx.try_recv().is_ok() {
            return Ok(ToolExecutionResult::Cancelled);
        }
        let entry = entry.map_err(|error| ToolError::ExecutionFailed(error.to_string()))?;
        let entry_path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| ToolError::ExecutionFailed(error.to_string()))?;
        let kind = if file_type.is_dir() {
            "directory"
        } else if file_type.is_file() {
            "file"
        } else if file_type.is_symlink() {
            "symlink"
        } else {
            "other"
        };
        let _ = status_tx.send(ToolInvocationStatus::Running(
            self::serde_json::json!({
                "path": entry_path.display().to_string(),
                "type": kind
            })
            .to_string(),
        ));

        if recursive && file_type.is_dir() {
            match collect_entries(&entry_path, recursive, status_tx, cancel_rx)? {
                ToolExecutionResult::Complete(()) => {}
                ToolExecutionResult::Cancelled => return Ok(ToolExecutionResult::Cancelled),
            }
        }
    }

    Ok(ToolExecutionResult::Complete(()))
}
