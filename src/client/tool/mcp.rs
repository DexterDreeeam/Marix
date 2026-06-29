use super::descriptor::ToolDescriptor;
use super::error::ToolError;

/// A connection to one MCP server that discovers the tools it exposes so they
/// can be registered alongside builtin and user tools.
pub trait McpServer {
    fn name(&self) -> String;

    fn discover(&self) -> Result<Vec<ToolDescriptor>, ToolError>;
}
