use crate::common::channel::ChannelError;
use crate::common::message::UserMessage;

pub struct AgentTask;

impl AgentTask {
    pub fn send(&mut self, _message: impl UserMessage) -> Result<(), ChannelError> {
        Err(ChannelError::Unsupported(
            "agent task sending is not implemented".to_owned(),
        ))
    }

    pub fn receive(&mut self) -> Result<Box<dyn UserMessage>, ChannelError> {
        Err(ChannelError::Unsupported(
            "agent task receiving is not implemented".to_owned(),
        ))
    }

    pub fn complete(&mut self) -> Result<(), ChannelError> {
        Err(ChannelError::Unsupported(
            "agent task completion is not implemented".to_owned(),
        ))
    }
}
