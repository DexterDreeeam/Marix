use crate::external::*;

use crate::{ExecutionEvent, ExecutionRequest, ExecutionSignature};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutorEvent {
    Execution(ExecutionSignature, ExecutionEvent),
    ExecutionCreate(ExecutionRequest),
    ToolQuery,
}
