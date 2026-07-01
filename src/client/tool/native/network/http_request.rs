use std::{sync::mpsc, thread};

use crate::client::tool::{
    Tool, ToolCategory, ToolError, ToolInvocation, ToolInvocationStatus, ToolRuntime, ToolSchema,
    ToolType,
};
use crate::common::config::Platform;
use crate::common::external::*;

pub struct HttpRequestTool;

impl HttpRequestTool {
    pub const NAME: &'static str = "native_http_request";
    pub const DESCRIPTION: &'static str = "Send a native HTTP request.";
    pub const SCHEMA: &'static str = r#"{"type":"object","properties":{"url":{"type":"string"},"method":{"type":"string"},"body":{"type":"string"}},"required":["url"],"additionalProperties":false}"#;
}

impl Tool for HttpRequestTool {
    fn tool_type(&self) -> ToolType {
        ToolType::Primary
    }

    fn platform(&self) -> Platform {
        Platform::All
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Network
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
            match send_request(arguments) {
                Ok(message) => {
                    if cancel_rx.try_recv().is_ok() {
                        let _ = status_tx.send(ToolInvocationStatus::Cancelled);
                        return;
                    }
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

struct HttpRequestArguments {
    url: String,
    method: String,
    body: Option<String>,
}

fn parse_arguments(arguments: &str) -> Result<HttpRequestArguments, ToolError> {
    let value: serde_json::Value = serde_json::from_str(arguments)
        .map_err(|error| ToolError::InvalidArguments(error.to_string()))?;
    let url = value
        .get("url")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| ToolError::InvalidArguments("missing string field: url".to_string()))?;
    let method = value
        .get("method")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("GET")
        .to_uppercase();
    let body = value
        .get("body")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);

    Ok(HttpRequestArguments { url, method, body })
}

fn send_request(arguments: HttpRequestArguments) -> Result<String, ToolError> {
    let client = reqwest::blocking::Client::new();
    let request = match arguments.method.as_str() {
        "GET" => client.get(&arguments.url),
        "POST" => client
            .post(&arguments.url)
            .body(arguments.body.unwrap_or_default()),
        "PUT" => client
            .put(&arguments.url)
            .body(arguments.body.unwrap_or_default()),
        "PATCH" => client
            .patch(&arguments.url)
            .body(arguments.body.unwrap_or_default()),
        "DELETE" => client.delete(&arguments.url),
        "HEAD" => client.head(&arguments.url),
        method => {
            return Err(ToolError::InvalidArguments(format!(
                "unsupported HTTP method: {method}"
            )));
        }
    };

    let response = request.send().map_err(|error| {
        ToolError::ExecutionFailed(format!("failed to request {}: {error}", arguments.url))
    })?;
    let status = response.status().as_u16();
    let body = response.text().map_err(|error| {
        ToolError::ExecutionFailed(format!("failed to read response body: {error}"))
    })?;

    Ok(self::serde_json::json!({
        "status": status,
        "body": body
    })
    .to_string())
}
