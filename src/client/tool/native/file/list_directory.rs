use std::sync::mpsc::Receiver;

use crate::client::tool::{
    Tool, ToolCategory, ToolError, ToolInvocation, ToolOutcome, ToolPlatform, ToolPreview, ToolType,
};

pub struct ListDirectoryTool;

impl ListDirectoryTool {
    pub const PREVIEW: ToolPreview = ToolPreview {
        name: "native_list_directory",
        description: "List entries under a local directory.",
        schema: r#"{"type":"object","properties":{"path":{"type":"string"},"recursive":{"type":"boolean"}},"required":["path"],"additionalProperties":false}"#,
    };
}

impl Tool for ListDirectoryTool {
    fn tool_type(&self) -> ToolType {
        panic!("not implemented")
    }

    fn platforms(&self) -> &'static [ToolPlatform] {
        panic!("not implemented")
    }

    fn categories(&self) -> &'static [ToolCategory] {
        panic!("not implemented")
    }

    fn name(&self) -> &'static str {
        panic!("not implemented")
    }

    fn description(&self) -> &'static str {
        panic!("not implemented")
    }

    fn schema(&self) -> &'static str {
        panic!("not implemented")
    }

    fn invoke(&self, _invocation: ToolInvocation) -> Result<Receiver<ToolOutcome>, ToolError> {
        panic!("not implemented")
    }
}
