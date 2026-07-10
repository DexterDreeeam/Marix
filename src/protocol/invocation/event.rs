use crate::external::*;

use crate::ExecutionStatus;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvocationEvent {
    ExecutionCreate,
    Update(ExecutionStatus),
    Processing { seq: usize, content: String },
    Cancel,
}
