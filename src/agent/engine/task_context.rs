use crate::common::channel::SessionTaskId;
use crate::common::message::UserMessageEnvelope;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TaskRuntimeEvent {
    Status(TaskStatus),
    ModelRequest(String),
    ModelResponse(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TaskStatus {
    Created,
    Running,
    Stopped,
    Succeeded,
    Failed(String),
}

pub(crate) struct TaskContext;

impl TaskContext {
    pub(crate) fn task_id(&self) -> SessionTaskId {
        panic!("not implemented")
    }

    pub(crate) fn initial_message(&self) -> &UserMessageEnvelope {
        panic!("not implemented")
    }

    pub(crate) fn status(&self) -> TaskStatus {
        panic!("not implemented")
    }
}
