use std::{fs, path::Path, sync::mpsc, thread};

use crate::client::tool::{
    Tool, ToolCategory, ToolError, ToolInvocation, ToolInvocationStatus, ToolRuntime, ToolSchema,
    ToolType,
};
use crate::common::config::Platform;
use crate::common::external::*;

pub struct WriteFileTool;

impl WriteFileTool {
    pub const NAME: &'static str = "native_write_file";
    pub const DESCRIPTION: &'static str = "Write UTF-8 text content to a local file.";
    pub const SCHEMA: &'static str = r#"{"type":"object","properties":{"path":{"type":"string"},"content":{"type":"string"},"create_dirs":{"type":"boolean"}},"required":["path","content"],"additionalProperties":false}"#;
}

impl Tool for WriteFileTool {
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
            if cancel_rx.try_recv().is_ok() {
                let _ = status_tx.send(ToolInvocationStatus::Cancelled);
                return;
            }
            match write_file(arguments, &cancel_rx) {
                Ok(message) => {
                    let _ = status_tx.send(ToolInvocationStatus::Running(message));
                    let _ = status_tx.send(ToolInvocationStatus::Complete);
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

struct WriteFileArguments {
    path: String,
    content: String,
    create_dirs: bool,
}

fn parse_arguments(arguments: &str) -> Result<WriteFileArguments, ToolError> {
    let value: serde_json::Value = serde_json::from_str(arguments)
        .map_err(|error| ToolError::InvalidArguments(error.to_string()))?;
    let path = required_string(&value, "path")?;
    let content = required_string(&value, "content")?;
    let create_dirs = value
        .get("create_dirs")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    Ok(WriteFileArguments {
        path,
        content,
        create_dirs,
    })
}

fn write_file(
    arguments: WriteFileArguments,
    cancel_rx: &mpsc::Receiver<()>,
) -> Result<String, ToolError> {
    if arguments.create_dirs {
        if let Some(parent) = Path::new(&arguments.path).parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|error| {
                    ToolError::ExecutionFailed(format!(
                        "failed to create parent directories for {}: {error}",
                        arguments.path
                    ))
                })?;
            }
        }
    }
    if cancel_rx.try_recv().is_ok() {
        return Err(ToolError::Denied(
            "tool invocation was cancelled".to_string(),
        ));
    }

    fs::write(&arguments.path, arguments.content.as_bytes()).map_err(|error| {
        ToolError::ExecutionFailed(format!("failed to write {}: {error}", arguments.path))
    })?;

    Ok(self::serde_json::json!({
        "path": arguments.path,
        "bytes": arguments.content.len()
    })
    .to_string())
}

fn required_string(value: &serde_json::Value, field: &str) -> Result<String, ToolError> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| ToolError::InvalidArguments(format!("missing string field: {field}")))
}
