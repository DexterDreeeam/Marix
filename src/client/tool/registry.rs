use super::descriptor::ToolDescriptor;
use super::error::ToolError;
use super::mcp::McpServer;
use super::tool::Tool;

/// Collection of available tools, keyed by name. Builtins are registered at
/// startup; user tools are registered at runtime. Advertised to the engine as a
/// descriptor list.
pub struct ToolRegistry;

impl ToolRegistry {
    pub fn new() -> Self {
        panic!("not implemented")
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) -> Result<(), ToolError> {
        panic!("not implemented")
    }

    pub fn register_mcp_server(&mut self, server: Box<dyn McpServer>) -> Result<(), ToolError> {
        panic!("not implemented")
    }

    pub fn descriptors(&self) -> Vec<ToolDescriptor> {
        panic!("not implemented")
    }

    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        panic!("not implemented")
    }
}
