use crate::common::external::*;

/// Public, model-facing description of a single tool: the metadata an engine
/// advertises to a model so it can decide when and how to call the tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDescriptor {
    pub name: String,
    pub description: String,
    pub parameters: ToolParameterSchema,
    pub source: ToolSource,
}

/// JSON-Schema-shaped declaration of a tool's accepted arguments. Carried as a
/// serialized schema document so it can cross the session boundary unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolParameterSchema {
    pub schema: String,
}

/// Where a registered tool originates: process built-ins, caller-supplied user
/// tools, or tools discovered from an MCP server (tagged by server name).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolSource {
    Builtin,
    User,
    Mcp { server: String },
}
