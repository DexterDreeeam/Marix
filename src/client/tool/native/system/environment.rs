use std::{env, sync::mpsc, thread};

use crate::client::tool::{
    Tool, ToolCategory, ToolError, ToolInvocation, ToolInvocationStatus, ToolPreview, ToolRuntime,
    ToolType,
};
use crate::common::config::Platform;
use crate::common::external::*;

pub struct EnvironmentTool;

impl EnvironmentTool {
    pub const PREVIEW: ToolPreview = ToolPreview {
        name: "native_environment",
        description: "Read selected local environment variables.",
        schema: r#"{"type":"object","properties":{"names":{"type":"array","items":{"type":"string"}}},"required":["names"],"additionalProperties":false}"#,
    };
}

impl Tool for EnvironmentTool {
    fn tool_type(&self) -> ToolType {
        ToolType::Native
    }

    fn platform(&self) -> Platform {
        Platform::All
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Environment
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
        let names = parse_names(&invocation.arguments)?;
        let (status_tx, status_rx) = mpsc::channel();
        let (cancel_tx, cancel_rx) = mpsc::channel();

        thread::spawn(move || {
            let _ = status_tx.send(ToolInvocationStatus::Started);
            for name in names {
                if cancel_rx.try_recv().is_ok() {
                    let _ = status_tx.send(ToolInvocationStatus::Cancelled);
                    return;
                }
                let _ = status_tx.send(ToolInvocationStatus::Running(
                    self::serde_json::json!({
                        "name": name,
                        "value": env::var(&name).ok()
                    })
                    .to_string(),
                ));
            }
            let _ = status_tx.send(ToolInvocationStatus::Complete);
        });

        Ok(ToolRuntime::new(status_rx, cancel_tx))
    }
}

// -- Private -- //

fn parse_names(arguments: &str) -> Result<Vec<String>, ToolError> {
    let value: serde_json::Value = serde_json::from_str(arguments)
        .map_err(|error| ToolError::InvalidArguments(error.to_string()))?;
    let names = value
        .get("names")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| ToolError::InvalidArguments("missing array field: names".to_string()))?;
    let mut parsed_names = Vec::with_capacity(names.len());
    for name in names {
        let name = name.as_str().ok_or_else(|| {
            ToolError::InvalidArguments("names must contain only strings".to_string())
        })?;
        parsed_names.push(name.to_string());
    }

    Ok(parsed_names)
}
