use crate::common::external::*;

/// Failures surfaced while dispatching a tool invocation, before any per-chunk
/// outcomes start streaming back over the channel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutorError {
    Unknown(String),
    InvalidArguments(String),
    Denied(String),
    DispatchFailed(String),
}
