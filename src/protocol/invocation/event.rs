use crate::external::*;

use crate::{ExecutionEvent, ExecutionStatus};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvocationEvent {
    Execution(ExecutionEvent),
    ExecutionCreate,
    ExecutionUpdate(ExecutionStatus),
    Cancel,
}
