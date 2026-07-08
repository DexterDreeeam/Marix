use crate::TaskResult;
use crate::external::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Created,
    Started,
    Canceled,
    Succeed(TaskResult),
    Failed { reason: String },
}
