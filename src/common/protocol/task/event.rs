use crate::external::*;
use crate::protocol::{StepEvent, StepSignature, TaskStatus};

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
