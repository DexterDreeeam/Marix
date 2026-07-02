use std::{sync::mpsc, thread};

use crate::client::tool::{
    Tool, ToolCategory, ToolError, ToolInvocation, ToolExecutionStatus, ToolRuntime, ToolSchema,
    ToolType,
};
use crate::common::config::Platform;
use crate::common::external::*;

pub struct ImageInspectTool;

impl ImageInspectTool {
    pub const NAME: &'static str = "native_image_inspect";
    pub const DESCRIPTION: &'static str = "Read image metadata from a local image file.";
    pub const SCHEMA: &'static str = r#"{"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false}"#;
}

impl Tool for ImageInspectTool {
    fn tool_type(&self) -> ToolType {
        ToolType::Native
    }

    fn platform(&self) -> Platform {
        Platform::All
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Image
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
        let path = required_string(&invocation.parameter.payload, "path")?;
        let (status_tx, status_rx) = mpsc::channel();
        let (cancel_tx, cancel_rx) = mpsc::channel();

        thread::spawn(move || {
            let _ = status_tx.send(ToolExecutionStatus::Started);
            if cancel_rx.try_recv().is_ok() {
                let _ = status_tx.send(ToolExecutionStatus::Cancelled);
                return;
            }
            match inspect_image(&path) {
                Ok(message) => {
                    let _ = status_tx.send(ToolExecutionStatus::Running(message));
                    let _ = status_tx.send(ToolExecutionStatus::Complete { output: None });
                }
                Err(error) => {
                    let _ = status_tx.send(ToolExecutionStatus::Failed(error));
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

fn inspect_image(path: &str) -> Result<String, ToolError> {
    let reader = image::ImageReader::open(path)
        .map_err(|error| ToolError::ExecutionFailed(format!("failed to open {path}: {error}")))?
        .with_guessed_format()
        .map_err(|error| {
            ToolError::ExecutionFailed(format!("failed to inspect {path}: {error}"))
        })?;
    let format = reader
        .format()
        .map(|format| format!("{format:?}"))
        .unwrap_or_else(|| "unknown".to_string());
    let image = reader
        .decode()
        .map_err(|error| ToolError::ExecutionFailed(format!("failed to decode {path}: {error}")))?;

    Ok(self::serde_json::json!({
        "path": path,
        "format": format,
        "width": image.width(),
        "height": image.height(),
        "color": format!("{:?}", image.color())
    })
    .to_string())
}
