use std::{fs, path::Path, sync::mpsc, thread};

use crate::client::tool::{
    Tool, ToolCategory, ToolError, ToolExecutionResult, ToolInvocation, ToolInvocationStatus,
    ToolSchema, ToolRuntime, ToolType,
};
use crate::common::config::Platform;
use crate::common::external::*;

pub struct SearchTextTool;

impl SearchTextTool {
    pub const NAME: &'static str = "native_search_text";
    pub const DESCRIPTION: &'static str = "Search text under a local directory or file path.";
    pub const SCHEMA: &'static str = r#"{"type":"object","properties":{"path":{"type":"string"},"query":{"type":"string"},"case_sensitive":{"type":"boolean"}},"required":["path","query"],"additionalProperties":false}"#;
}

impl Tool for SearchTextTool {
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
            match search_path(
                Path::new(&arguments.path),
                &arguments,
                &status_tx,
                &cancel_rx,
            ) {
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

struct SearchTextArguments {
    path: String,
    query: String,
    case_sensitive: bool,
}

fn parse_arguments(arguments: &str) -> Result<SearchTextArguments, ToolError> {
    let value: serde_json::Value = serde_json::from_str(arguments)
        .map_err(|error| ToolError::InvalidArguments(error.to_string()))?;
    let path = required_string(&value, "path")?;
    let query = required_string(&value, "query")?;
    let case_sensitive = value
        .get("case_sensitive")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    Ok(SearchTextArguments {
        path,
        query,
        case_sensitive,
    })
}

fn search_path(
    path: &Path,
    arguments: &SearchTextArguments,
    status_tx: &mpsc::Sender<ToolInvocationStatus>,
    cancel_rx: &mpsc::Receiver<()>,
) -> Result<ToolExecutionResult<()>, ToolError> {
    if cancel_rx.try_recv().is_ok() {
        return Ok(ToolExecutionResult::Cancelled);
    }

    if path.is_dir() {
        for entry in fs::read_dir(path).map_err(|error| {
            ToolError::ExecutionFailed(format!("failed to read {}: {error}", path.display()))
        })? {
            let entry = entry.map_err(|error| ToolError::ExecutionFailed(error.to_string()))?;
            match search_path(&entry.path(), arguments, status_tx, cancel_rx)? {
                ToolExecutionResult::Complete(()) => {}
                ToolExecutionResult::Cancelled => return Ok(ToolExecutionResult::Cancelled),
            }
        }
        return Ok(ToolExecutionResult::Complete(()));
    }

    if path.is_file() {
        match search_file(path, arguments, status_tx, cancel_rx)? {
            ToolExecutionResult::Complete(()) => {}
            ToolExecutionResult::Cancelled => return Ok(ToolExecutionResult::Cancelled),
        }
    }

    Ok(ToolExecutionResult::Complete(()))
}

fn search_file(
    path: &Path,
    arguments: &SearchTextArguments,
    status_tx: &mpsc::Sender<ToolInvocationStatus>,
    cancel_rx: &mpsc::Receiver<()>,
) -> Result<ToolExecutionResult<()>, ToolError> {
    let content = fs::read_to_string(path).map_err(|error| {
        ToolError::ExecutionFailed(format!("failed to read {}: {error}", path.display()))
    })?;
    let query = if arguments.case_sensitive {
        arguments.query.clone()
    } else {
        arguments.query.to_lowercase()
    };

    for (index, line) in content.lines().enumerate() {
        if cancel_rx.try_recv().is_ok() {
            return Ok(ToolExecutionResult::Cancelled);
        }
        let haystack = if arguments.case_sensitive {
            line.to_string()
        } else {
            line.to_lowercase()
        };
        if haystack.contains(&query) {
            let _ = status_tx.send(ToolInvocationStatus::Running(
                self::serde_json::json!({
                    "path": path.display().to_string(),
                    "line_number": index + 1,
                    "line": line
                })
                .to_string(),
            ));
        }
    }

    Ok(ToolExecutionResult::Complete(()))
}

fn required_string(value: &serde_json::Value, field: &str) -> Result<String, ToolError> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| ToolError::InvalidArguments(format!("missing string field: {field}")))
}
