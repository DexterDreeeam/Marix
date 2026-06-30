use std::sync::mpsc::Receiver;

use crate::client::tool::{
    Tool, ToolCategory, ToolError, ToolInvocation, ToolOutcome, ToolPlatform, ToolPreview, ToolType,
};

pub struct ImageInspectTool;

impl ImageInspectTool {
    pub const PREVIEW: ToolPreview = ToolPreview {
        name: "native_image_inspect",
        description: "Read image metadata from a local image file.",
        schema: r#"{"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false}"#,
    };
}

impl Tool for ImageInspectTool {
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
