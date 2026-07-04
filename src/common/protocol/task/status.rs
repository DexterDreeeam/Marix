use crate::external::*;
use crate::protocol::TaskResult;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Started,
    Update { content: String },
    Canceled,
    Succeed(TaskResult),
    Failed { reason: String },
}
