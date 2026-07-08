use crate::external::*;

use crate::{ExecutorEvent, TaskEvent, TaskRequest, TaskSignature, TaskStatus};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionEvent {
    Task(TaskSignature, TaskEvent),
    TaskCreate(TaskRequest),
    TaskUpdate(TaskStatus),
    Executor(ExecutorEvent),
}
