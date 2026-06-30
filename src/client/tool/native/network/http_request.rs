use std::sync::mpsc::Receiver;

use crate::client::tool::{
    Tool, ToolCategory, ToolError, ToolInvocation, ToolOutcome, ToolPlatform, ToolPreview, ToolType,
};

pub struct HttpRequestTool;

impl HttpRequestTool {
    pub const PREVIEW: ToolPreview = ToolPreview {
        name: "native_http_request",
        description: "Send a native HTTP request.",
        schema: r#"{"type":"object","properties":{"url":{"type":"string"},"method":{"type":"string"},"body":{"type":"string"}},"required":["url"],"additionalProperties":false}"#,
    };
}

impl Tool for HttpRequestTool {
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
