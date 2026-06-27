use std::sync::mpsc;

use crate::agent::frontdoor::AgentTask;
use crate::common::channel::ChannelError;
use crate::common::channel::SessionTaskId;
use crate::common::message::ChatMessage;
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

pub(crate) struct TaskContext {
    task: AgentTask,
    initial_message: UserMessageEnvelope,
    status: TaskStatus,
    runtime_tx: Option<mpsc::Sender<TaskRuntimeEvent>>,
}

impl TaskContext {
    pub(crate) fn new(mut task: AgentTask) -> Result<Self, ChannelError> {
        let initial_message = task.receive()?;
        Ok(Self {
            task,
            initial_message,
            status: TaskStatus::Created,
            runtime_tx: None,
        })
    }

    pub(crate) fn task_id(&self) -> SessionTaskId {
        self.task.task_id()
    }

    pub(crate) fn initial_message(&self) -> &UserMessageEnvelope {
        &self.initial_message
    }

    pub(crate) fn status(&self) -> TaskStatus {
        self.status.clone()
    }
}

impl TaskContext {
    pub(super) fn attach_runtime_sender(&mut self, runtime_tx: mpsc::Sender<TaskRuntimeEvent>) {
        self.runtime_tx = Some(runtime_tx);
    }

    pub(super) fn publish_status(&mut self, status: TaskStatus) {
        self.status = status.clone();
        self.publish_runtime_event(TaskRuntimeEvent::Status(status));
    }

    pub(super) fn publish_model_request(&self, prompt: String) {
        self.publish_runtime_event(TaskRuntimeEvent::ModelRequest(prompt));
    }

    pub(super) fn publish_model_response(&self, content: String) {
        self.publish_runtime_event(TaskRuntimeEvent::ModelResponse(content));
    }

    pub(super) fn send_model_content(&mut self, content: String) -> Result<(), ChannelError> {
        self.task.send(ChatMessage { content })
    }

    pub(super) fn complete_client_task(&mut self) -> Result<(), ChannelError> {
        self.task.complete()
    }
}

impl TaskContext {
    fn publish_runtime_event(&self, event: TaskRuntimeEvent) {
        if let Some(runtime_tx) = &self.runtime_tx {
            let _ = runtime_tx.send(event);
        }
    }
}
