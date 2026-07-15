use crate::external::*;

use crate::{ExecutionSignature, ExecutionStatus};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvocationEvent {
    Update(ExecutionSignature, ExecutionStatus),
    Processing {
        execution: ExecutionSignature,
        seq: usize,
        content: String,
    },
    Cancel,
}
