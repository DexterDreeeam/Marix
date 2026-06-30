use std::sync::mpsc::Receiver;

use crate::client::tool::{
    Tool, ToolCategory, ToolError, ToolInvocation, ToolOutcome, ToolPlatform, ToolPreview, ToolType,
};

pub struct SearchTextTool;

impl SearchTextTool {
    pub const PREVIEW: ToolPreview = ToolPreview {
        name: "native_search_text",
        description: "Search text under a local directory or file path.",
        schema: r#"{"type":"object","properties":{"path":{"type":"string"},"query":{"type":"string"},"case_sensitive":{"type":"boolean"}},"required":["path","query"],"additionalProperties":false}"#,
    };
}

impl Tool for SearchTextTool {
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
