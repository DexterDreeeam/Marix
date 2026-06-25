use crate::common::channel::ChannelError;
use crate::common::message::UserMessage;

pub struct AgentTask;

impl AgentTask {
    pub fn send(&mut self, _message: impl UserMessage) -> Result<(), ChannelError> {
        panic!("not implemented")
    }

    pub fn receive(&mut self) -> Result<Box<dyn UserMessage>, ChannelError> {
        panic!("not implemented")
    }

    pub fn complete(&mut self) -> Result<(), ChannelError> {
        panic!("not implemented")
    }
}
