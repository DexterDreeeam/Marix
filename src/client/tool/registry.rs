use super::category::CategoryPreview;
use super::error::ToolError;
use super::tool::{Tool, ToolPreview};

pub struct DefaultPreview {
    pub primary_tool_previews: Vec<ToolPreview>,
    pub category_previews: Vec<CategoryPreview>,
}

/// Collection of available tools, keyed by name. Builtins are registered at
/// startup; user tools are registered at runtime.
pub struct ToolRegistry;

impl ToolRegistry {
    pub fn new() -> Self {
        panic!("not implemented")
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) -> Result<(), ToolError> {
        panic!("not implemented")
    }

    pub fn default_preview(&self) -> DefaultPreview {
        panic!("not implemented")
    }

    pub fn tool_preview(&self) -> Vec<ToolPreview> {
        panic!("not implemented")
    }

    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        panic!("not implemented")
    }
}
