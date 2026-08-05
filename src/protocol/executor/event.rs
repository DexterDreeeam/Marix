use crate::external::*;

use crate::{
    ContinuationRequest, ExecutionEvent, ExecutionRequest,
    ExecutionSignature,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutorEvent {
    Continuation(ContinuationRequest),
    Execution(ExecutionSignature, ExecutionEvent),
    ExecutionCreate(ExecutionRequest),
    ToolQuery,
}
