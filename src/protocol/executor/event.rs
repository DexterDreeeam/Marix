use crate::external::*;

use crate::{ExecutionEvent, ExecutionRequest, ExecutionSignature, ExecutionStatus};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutorEvent {
    Execution(ExecutionSignature, ExecutionEvent),
    ExecutionCreate(ExecutionRequest),
    ExecutionUpdate(ExecutionSignature, ExecutionStatus),
}
