use crate::external::*;
use crate::protocol::{TaskRequestBrief, TaskResult, TaskSignature};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskPreview {
    pub signature: TaskSignature,
    pub request: TaskRequestBrief,
    pub result: TaskResult,
}
