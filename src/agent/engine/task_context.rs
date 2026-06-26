use crate::agent::frontdoor::AgentTask;
use crate::common::channel::ChannelError;
use crate::common::message::{UserMessage, UserMessageEnvelope};

pub struct TaskContext {
    task: AgentTask,
}

impl TaskContext {
    pub(crate) fn new(task: AgentTask) -> Self {
        Self { task }
    }

    pub(crate) fn receive(&mut self) -> Result<UserMessageEnvelope, ChannelError> {
        self.task.receive()
    }

    pub(crate) fn send(&mut self, message: impl UserMessage) -> Result<(), ChannelError> {
        self.task.send(message)
    }

    pub(crate) fn complete(&mut self) -> Result<(), ChannelError> {
        self.task.complete()
    }
}
