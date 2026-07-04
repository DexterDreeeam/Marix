use crate::TaskResult;
use crate::external::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Started,
    Update { content: String },
    Canceled,
    Succeed(TaskResult),
    Failed { reason: String },
}
