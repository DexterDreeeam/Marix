use crate::common::external::*;

/// A tool's declared input schema, carried as a JSON Schema document. Shared
/// protocol type so the client advertises and the agent consumes the exact same
/// schema shape across the session boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolSchema {
    pub schema: String,
}

/// A tool-call parameter package: a JSON argument payload that must conform to a
/// tool's ToolSchema before the invocation is issued. Produced by the agent and
/// executed by the client, so it is the single shared parameter type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolParameter {
    pub payload: String,
}

impl ToolSchema {
    pub fn new(schema: impl Into<String>) -> Self {
        Self {
            schema: schema.into(),
        }
    }
}

impl ToolParameter {
    pub fn new(payload: impl Into<String>) -> Self {
        Self {
            payload: payload.into(),
        }
    }
}
