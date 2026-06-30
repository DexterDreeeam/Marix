use std::{env, sync::mpsc, thread};

use crate::client::tool::{
    Tool, ToolCategory, ToolError, ToolInvocation, ToolInvocationStatus, ToolPreview, ToolRuntime,
    ToolType,
};
use crate::common::config::Platform;
use crate::common::external::*;

pub struct SystemInfoTool;

impl SystemInfoTool {
    pub const PREVIEW: ToolPreview = ToolPreview {
        name: "native_system_info",
        description: "Report native operating system and machine information.",
        schema: r#"{"type":"object","properties":{},"additionalProperties":false}"#,
    };
}

impl Tool for SystemInfoTool {
    fn tool_type(&self) -> ToolType {
        ToolType::Native
    }

    fn platform(&self) -> Platform {
        Platform::All
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::System
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

    fn invoke(&self, _invocation: ToolInvocation) -> Result<ToolRuntime, ToolError> {
        let (status_tx, status_rx) = mpsc::channel();
        let (cancel_tx, cancel_rx) = mpsc::channel();

        thread::spawn(move || {
            let _ = status_tx.send(ToolInvocationStatus::Started);
            if cancel_rx.try_recv().is_ok() {
                let _ = status_tx.send(ToolInvocationStatus::Cancelled);
                return;
            }
            let current_dir = env::current_dir()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|error| format!("unavailable: {error}"));
            let current_exe = env::current_exe()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|error| format!("unavailable: {error}"));
            let _ = status_tx.send(ToolInvocationStatus::Running(
                self::serde_json::json!({
                    "os": env::consts::OS,
                    "family": env::consts::FAMILY,
                    "architecture": env::consts::ARCH,
                    "current_dir": current_dir,
                    "current_exe": current_exe
                })
                .to_string(),
            ));
            let _ = status_tx.send(ToolInvocationStatus::Complete);
        });

        Ok(ToolRuntime::new(status_rx, cancel_tx))
    }
}
