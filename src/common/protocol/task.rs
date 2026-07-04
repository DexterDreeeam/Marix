use crate::protocol::{StepEvent, StepSignature, TaskId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskSignature {
    pub name: String,
    pub id: TaskId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskEvent {
    Create { request: String },
    CreateFailed { reason: String },
    Query,
    Preview { content: String },
    Cancel,
    Status(TaskStatus),
    Step(StepSignature, StepEvent),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Started,
    Update { content: String },
    Canceled,
    Succeed,
    Failed { reason: String },
}
