use crate::TaskStatus;
use crate::external::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskEvent {
    Create { request: String },
    CreateFailed { reason: String },
    Query,
    Preview { content: String },
    Cancel,
    Status(TaskStatus),
}
