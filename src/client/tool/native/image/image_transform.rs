use std::sync::mpsc::Receiver;

use crate::client::tool::{
    Tool, ToolCategory, ToolError, ToolInvocation, ToolOutcome, ToolPlatform, ToolPreview, ToolType,
};

pub struct ImageTransformTool;

impl ImageTransformTool {
    pub const PREVIEW: ToolPreview = ToolPreview {
        name: "native_image_transform",
        description: "Transform a local image file into another image output.",
        schema: r#"{"type":"object","properties":{"input_path":{"type":"string"},"output_path":{"type":"string"},"operation":{"type":"string"}},"required":["input_path","output_path","operation"],"additionalProperties":false}"#,
    };
}

impl Tool for ImageTransformTool {
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
