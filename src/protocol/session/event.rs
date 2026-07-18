use crate::external::*;

use marix_common::System;

use crate::{ExecutorEvent, TaskEvent, TaskRequest, TaskSignature, TaskStatus, ToolPreview};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionEvent {
    SessionId(uuid::Uuid),
    Task(TaskSignature, TaskEvent),
    TaskCreate(TaskRequest),
    TaskUpdate(TaskStatus),
    ExecutorTools(System, Vec<ToolPreview>),
    Executor(ExecutorEvent),
}
