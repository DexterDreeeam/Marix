use std::{fs, sync::mpsc, thread};

use crate::client::tool::{
    Tool, ToolCategory, ToolError, ToolExecutionStatus, ToolInvocation, ToolSchema, ToolRuntime,
    ToolType,
};
use crate::common::config::Platform;
use crate::common::external::*;

pub struct ImageTransformTool;

impl ImageTransformTool {
    pub const NAME: &'static str = "native_image_transform";
    pub const DESCRIPTION: &'static str = "Transform a local image file into another image output.";
    pub const SCHEMA: &'static str = r#"{"type":"object","properties":{"input_path":{"type":"string"},"output_path":{"type":"string"},"operation":{"type":"string"}},"required":["input_path","output_path","operation"],"additionalProperties":false}"#;
}

impl Tool for ImageTransformTool {
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
        let arguments = parse_arguments(&invocation.parameter.payload)?;
        let (status_tx, status_rx) = mpsc::channel();
        let (cancel_tx, cancel_rx) = mpsc::channel();

        thread::spawn(move || {
            let _ = status_tx.send(ToolExecutionStatus::Started);
            if cancel_rx.try_recv().is_ok() {
                let _ = status_tx.send(ToolExecutionStatus::Cancelled);
                return;
            }
            match transform_image(&arguments, &cancel_rx) {
                Ok(status) => {
                    let _ = status_tx.send(status);
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

struct ImageTransformArguments {
    input_path: String,
    output_path: String,
    operation: String,
}

fn parse_arguments(arguments: &str) -> Result<ImageTransformArguments, ToolError> {
    let value: serde_json::Value = serde_json::from_str(arguments)
        .map_err(|error| ToolError::InvalidArguments(error.to_string()))?;

    Ok(ImageTransformArguments {
        input_path: required_string(&value, "input_path")?,
        output_path: required_string(&value, "output_path")?,
        operation: required_string(&value, "operation")?.to_lowercase(),
    })
}

fn transform_image(
    arguments: &ImageTransformArguments,
    cancel_rx: &mpsc::Receiver<()>,
) -> Result<ToolExecutionStatus, ToolError> {
    if arguments.operation == "copy" {
        fs::copy(&arguments.input_path, &arguments.output_path).map_err(|error| {
            ToolError::ExecutionFailed(format!(
                "failed to copy {} to {}: {error}",
                arguments.input_path, arguments.output_path
            ))
        })?;
        return Ok(ToolExecutionStatus::Complete {
            output: Some(transform_result(arguments)),
        });
    }
    if cancel_rx.try_recv().is_ok() {
        return Ok(ToolExecutionStatus::Cancelled);
    }

    let image = image::ImageReader::open(&arguments.input_path)
        .map_err(|error| {
            ToolError::ExecutionFailed(format!("failed to open {}: {error}", arguments.input_path))
        })?
        .decode()
        .map_err(|error| {
            ToolError::ExecutionFailed(format!(
                "failed to decode {}: {error}",
                arguments.input_path
            ))
        })?;
    if cancel_rx.try_recv().is_ok() {
        return Ok(ToolExecutionStatus::Cancelled);
    }

    let transformed = match arguments.operation.as_str() {
        "grayscale" => image.grayscale(),
        "flip_horizontal" => image.fliph(),
        "flip_vertical" => image.flipv(),
        "rotate90" => image.rotate90(),
        "rotate180" => image.rotate180(),
        "rotate270" => image.rotate270(),
        operation => {
            return Err(ToolError::InvalidArguments(format!(
                "unsupported image operation: {operation}"
            )));
        }
    };
    transformed.save(&arguments.output_path).map_err(|error| {
        ToolError::ExecutionFailed(format!("failed to save {}: {error}", arguments.output_path))
    })?;

    Ok(ToolExecutionStatus::Complete {
        output: Some(transform_result(arguments)),
    })
}

fn transform_result(arguments: &ImageTransformArguments) -> String {
    self::serde_json::json!({
        "input_path": arguments.input_path,
        "output_path": arguments.output_path,
        "operation": arguments.operation
    })
    .to_string()
}

fn required_string(value: &serde_json::Value, field: &str) -> Result<String, ToolError> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| ToolError::InvalidArguments(format!("missing string field: {field}")))
}
