use std::sync::{mpsc, Arc, Mutex};

use crate::agent::frontdoor::AgentTask;
use crate::common::channel::SessionTaskId;
use crate::common::message::RequestMessageEnvelope;

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

#[derive(Clone)]
pub(crate) struct TaskContext {
    pub(super) task_id: SessionTaskId,
    pub(super) initial_message: Arc<RequestMessageEnvelope>,
    pub(super) task: Arc<Mutex<AgentTask>>,
    pub(super) status: Arc<Mutex<TaskStatus>>,
    pub(super) runtime_tx: Arc<Mutex<Option<mpsc::Sender<TaskRuntimeEvent>>>>,
}

impl TaskContext {
    pub(crate) fn task_id(&self) -> SessionTaskId {
        self.task_id
    }

    pub(crate) fn initial_message(&self) -> &RequestMessageEnvelope {
        self.initial_message.as_ref()
    }

    pub(crate) fn status(&self) -> TaskStatus {
        self.status
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }
}
