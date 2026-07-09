use crate::external::*;

use crate::ExecutionStatus;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvocationEvent {
    ExecutionCreate,
    ExecutionUpdate(ExecutionStatus),
    Cancel,
}
