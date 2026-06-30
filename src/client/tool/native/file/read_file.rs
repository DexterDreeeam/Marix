use std::{fs, sync::mpsc, thread};

use crate::client::tool::{
    Tool, ToolCategory, ToolError, ToolInvocation, ToolInvocationStatus, ToolPreview, ToolRuntime,
    ToolType,
};
use crate::common::config::Platform;
use crate::common::external::*;

pub struct ReadFileTool;

impl ReadFileTool {
    pub const PREVIEW: ToolPreview = ToolPreview {
        name: "native_read_file",
        description: "Read a UTF-8 text file from the local file system.",
        schema: r#"{"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false}"#,
    };
}

impl Tool for ReadFileTool {
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
        Self::PREVIEW.name
    }

    fn description(&self) -> &'static str {
        Self::PREVIEW.description
    }

    fn schema(&self) -> &'static str {
        Self::PREVIEW.schema
    }

    fn invoke(&self, invocation: ToolInvocation) -> Result<ToolRuntime, ToolError> {
        let path = required_string(&invocation.arguments, "path")?;
        let (status_tx, status_rx) = mpsc::channel();
        let (cancel_tx, cancel_rx) = mpsc::channel();

        thread::spawn(move || {
            let _ = status_tx.send(ToolInvocationStatus::Started);
            if cancel_rx.try_recv().is_ok() {
                let _ = status_tx.send(ToolInvocationStatus::Cancelled);
                return;
            }
            match fs::read_to_string(&path) {
                Ok(content) => {
                    let _ = status_tx.send(ToolInvocationStatus::Running(content));
                    let _ = status_tx.send(ToolInvocationStatus::Complete);
                }
                Err(error) => {
                    let _ = status_tx.send(ToolInvocationStatus::Failed(
                        ToolError::ExecutionFailed(format!("failed to read {path}: {error}")),
                    ));
                }
            }
        });

        Ok(ToolRuntime::new(status_rx, cancel_tx))
    }
}

// -- Private -- //

fn required_string(arguments: &str, field: &str) -> Result<String, ToolError> {
    let value: serde_json::Value = serde_json::from_str(arguments)
        .map_err(|error| ToolError::InvalidArguments(error.to_string()))?;

    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| ToolError::InvalidArguments(format!("missing string field: {field}")))
}
