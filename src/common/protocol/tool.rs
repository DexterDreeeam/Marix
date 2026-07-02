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

/// One model-issued request to run a client-side tool, correlated by id so
/// streamed output can be routed back into the loop that requested it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolInvocation {
    pub signature: ToolSignature,
    pub parameter: ToolParameter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolType {
    Primary,
    Native,
    Skill,
    User,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolPreview {
    pub name: &'static str,
    pub description: &'static str,
    pub schema: ToolSchema,
}

/// Failures surfaced while resolving or executing a tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolError {
    Unknown(String),
    DuplicateName(String),
    InvalidArguments(String),
    ExecutionFailed(String),
    Denied(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolExecutionStatus {
    Pending { signature: ToolSignature },
    Started { signature: ToolSignature },
    Running { signature: ToolSignature, output: String },
    WaitingForCancel { signature: ToolSignature },
    Cancelled { signature: ToolSignature },
    Failed { signature: ToolSignature, error: ToolError },
    Complete {
        signature: ToolSignature,
        output: Option<String>,
    },
}

/// Stable identity of one tool invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolSignature {
    pub call_id: String,
    pub name: String,
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
