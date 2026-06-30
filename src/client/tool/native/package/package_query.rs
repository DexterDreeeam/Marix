use std::sync::mpsc::Receiver;

use crate::client::tool::{
    Tool, ToolCategory, ToolError, ToolInvocation, ToolOutcome, ToolPlatform, ToolPreview, ToolType,
};

pub struct PackageQueryTool;

impl PackageQueryTool {
    pub const PREVIEW: ToolPreview = ToolPreview {
        name: "native_package_query",
        description: "Query native package manager metadata for the current platform.",
        schema: r#"{"type":"object","properties":{"name":{"type":"string"},"include_versions":{"type":"boolean"}},"required":["name"],"additionalProperties":false}"#,
    };
}

impl Tool for PackageQueryTool {
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
