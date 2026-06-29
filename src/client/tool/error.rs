use crate::common::external::*;

/// Failures surfaced while resolving or executing a tool. Mirrors the typed,
/// stringly-detailed error style used by ChannelError.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolError {
    Unknown(String),
    DuplicateName(String),
    InvalidArguments(String),
    ExecutionFailed(String),
    Denied(String),
}
