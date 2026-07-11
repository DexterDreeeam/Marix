use crate::external::*;

use crate::{ExecutorEvent, TaskEvent, TaskRequest, TaskSignature, TaskStatus, ToolPreview};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionEvent {
    Task(TaskSignature, TaskEvent),
    TaskCreate(TaskRequest),
    TaskUpdate(TaskStatus),
    ExecutorTools(Vec<ToolPreview>),
    Executor(ExecutorEvent),
}
