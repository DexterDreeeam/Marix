use std::{env, sync::mpsc, thread};

use crate::client::tool::{
    Tool, ToolCategory, ToolError, ToolInvocation, ToolExecutionStatus, ToolRuntime, ToolSchema,
    ToolType,
};
use crate::common::config::Platform;
use crate::common::external::*;

pub struct SystemInfoTool;

impl SystemInfoTool {
    pub const NAME: &'static str = "native_system_info";
    pub const DESCRIPTION: &'static str = "Report native operating system and machine information.";
    pub const SCHEMA: &'static str = r#"{"type":"object","properties":{},"additionalProperties":false}"#;
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
        Self::NAME
    }

    fn description(&self) -> &'static str {
        Self::DESCRIPTION
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema::new(Self::SCHEMA)
    }

    fn invoke(&self, _invocation: ToolInvocation) -> Result<ToolRuntime, ToolError> {
        let (status_tx, status_rx) = mpsc::channel();
        let (cancel_tx, cancel_rx) = mpsc::channel();

        thread::spawn(move || {
            let _ = status_tx.send(ToolExecutionStatus::Started);
            if cancel_rx.try_recv().is_ok() {
                let _ = status_tx.send(ToolExecutionStatus::Cancelled);
                return;
            }
            let current_dir = env::current_dir()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|error| format!("unavailable: {error}"));
            let current_exe = env::current_exe()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|error| format!("unavailable: {error}"));
            let _ = status_tx.send(ToolExecutionStatus::Running(
                self::serde_json::json!({
                    "os": env::consts::OS,
                    "family": env::consts::FAMILY,
                    "architecture": env::consts::ARCH,
                    "current_dir": current_dir,
                    "current_exe": current_exe
                })
                .to_string(),
            ));
            let _ = status_tx.send(ToolExecutionStatus::Complete { output: None });
        });

        Ok(ToolRuntime::new(status_rx, cancel_tx))
    }
}
