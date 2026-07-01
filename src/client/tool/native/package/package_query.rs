use std::{
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::Duration,
};

use crate::client::tool::{
    Tool, ToolCategory, ToolError, ToolExecutionResult, ToolInvocation, ToolInvocationStatus,
    ToolSchema, ToolRuntime, ToolType,
};
use crate::common::config::Platform;
use crate::common::external::*;

pub struct PackageQueryTool;

impl PackageQueryTool {
    pub const NAME: &'static str = "native_package_query";
    pub const DESCRIPTION: &'static str =
        "Query native package manager metadata for the current platform.";
    pub const SCHEMA: &'static str = r#"{"type":"object","properties":{"name":{"type":"string"},"include_versions":{"type":"boolean"}},"required":["name"],"additionalProperties":false}"#;
}

impl Tool for PackageQueryTool {
    fn tool_type(&self) -> ToolType {
        ToolType::Native
    }

    fn platform(&self) -> Platform {
        Platform::Win
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Package
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
        if !cfg!(windows) {
            return Err(ToolError::Denied(
                "native_package_query is currently only supported on Windows".to_string(),
            ));
        }

        let arguments = parse_arguments(&invocation.parameter.payload)?;
        let (status_tx, status_rx) = mpsc::channel();
        let (cancel_tx, cancel_rx) = mpsc::channel();
        thread::spawn(move || {
            let _ = status_tx.send(ToolInvocationStatus::Started);
            if cancel_rx.try_recv().is_ok() {
                let _ = status_tx.send(ToolInvocationStatus::Cancelled);
                return;
            }
            match run_get_package(&arguments, &cancel_rx) {
                Ok(ToolExecutionResult::Complete(message)) => {
                    let _ = status_tx.send(ToolInvocationStatus::Running(message));
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

struct PackageQueryArguments {
    name: String,
    include_versions: bool,
}

fn parse_arguments(arguments: &str) -> Result<PackageQueryArguments, ToolError> {
    let value: serde_json::Value = serde_json::from_str(arguments)
        .map_err(|error| ToolError::InvalidArguments(error.to_string()))?;
    let name = value
        .get("name")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| ToolError::InvalidArguments("missing string field: name".to_string()))?;
    let include_versions = value
        .get("include_versions")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    Ok(PackageQueryArguments {
        name,
        include_versions,
    })
}

fn run_get_package(
    arguments: &PackageQueryArguments,
    cancel_rx: &mpsc::Receiver<()>,
) -> Result<ToolExecutionResult<String>, ToolError> {
    let selector = if arguments.include_versions {
        "Name,Version,ProviderName,Source"
    } else {
        "Name,ProviderName,Source"
    };
    let script = format!(
        "$name = $args[0]; @(Get-Package -Name \"*$name*\" -ErrorAction SilentlyContinue | Select-Object {selector}) | ConvertTo-Json -Depth 3"
    );
    let mut child = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &script,
            &arguments.name,
        ])
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

            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            return if stdout.is_empty() {
                Ok(ToolExecutionResult::Complete("[]".to_string()))
            } else {
                Ok(ToolExecutionResult::Complete(stdout))
            };
        }
        thread::sleep(Duration::from_millis(10));
    }
}
