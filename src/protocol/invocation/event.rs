use marix_common::ActorStatus;

use crate::external::*;

use crate::{ExecutionResult, ExecutionSignature, RelayResult, RelaySignature};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvocationEvent {
    Continuation {
        content: String,
        continuation_cursor: Option<String>,
    },
    ContinuationFailed {
        reason: String,
    },
    Update(ExecutionSignature, ActorStatus<ExecutionResult>),
    Processing {
        execution: ExecutionSignature,
        seq: usize,
        content: String,
    },
    SummarizeUpdate(RelaySignature, ActorStatus<RelayResult>),
    Cancel,
}
