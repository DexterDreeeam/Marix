use crate::external::*;
use crate::{TaskRequestBrief, TaskResult};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskPreview {
    pub request: TaskRequestBrief,
    pub result: TaskResult,
}
