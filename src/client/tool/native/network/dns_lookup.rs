use std::{collections::BTreeSet, net::ToSocketAddrs, sync::mpsc, thread};

use crate::client::tool::{
    Tool, ToolCategory, ToolError, ToolInvocation, ToolInvocationStatus, ToolPreview, ToolRuntime,
    ToolType,
};
use crate::common::config::Platform;
use crate::common::external::*;

pub struct DnsLookupTool;

impl DnsLookupTool {
    pub const PREVIEW: ToolPreview = ToolPreview {
        name: "native_dns_lookup",
        description: "Resolve a hostname with the native DNS resolver.",
        schema: r#"{"type":"object","properties":{"host":{"type":"string"}},"required":["host"],"additionalProperties":false}"#,
    };
}

impl Tool for DnsLookupTool {
    fn tool_type(&self) -> ToolType {
        ToolType::Native
    }

    fn platform(&self) -> Platform {
        Platform::All
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Network
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
        let host = required_string(&invocation.arguments, "host")?;
        let (status_tx, status_rx) = mpsc::channel();
        let (cancel_tx, cancel_rx) = mpsc::channel();

        thread::spawn(move || {
            let _ = status_tx.send(ToolInvocationStatus::Started);
            if cancel_rx.try_recv().is_ok() {
                let _ = status_tx.send(ToolInvocationStatus::Cancelled);
                return;
            }
            match resolve_host(&host) {
                Ok(addresses) => {
                    for address in addresses {
                        if cancel_rx.try_recv().is_ok() {
                            let _ = status_tx.send(ToolInvocationStatus::Cancelled);
                            return;
                        }
                        let _ = status_tx.send(ToolInvocationStatus::Running(address));
                    }
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

fn required_string(arguments: &str, field: &str) -> Result<String, ToolError> {
    let value: serde_json::Value = serde_json::from_str(arguments)
        .map_err(|error| ToolError::InvalidArguments(error.to_string()))?;

    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| ToolError::InvalidArguments(format!("missing string field: {field}")))
}

fn resolve_host(host: &str) -> Result<Vec<String>, ToolError> {
    let addresses = (host, 0)
        .to_socket_addrs()
        .map_err(|error| ToolError::ExecutionFailed(format!("failed to resolve {host}: {error}")))?
        .map(|address| address.ip().to_string())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|address| self::serde_json::json!({ "address": address }).to_string())
        .collect();

    Ok(addresses)
}
