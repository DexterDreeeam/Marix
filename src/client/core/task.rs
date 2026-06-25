use crate::common::channel::ChannelError;
use crate::common::message::UserMessage;

pub struct ClientTask;

impl ClientTask {
    pub fn send(&mut self, _message: impl UserMessage) -> Result<(), ChannelError> {
        Err(ChannelError::Unsupported(
            "client task sending is not implemented".to_owned(),
        ))
    }

    pub fn receive(&mut self) -> Result<Box<dyn UserMessage>, ChannelError> {
        Err(ChannelError::Unsupported(
            "client task receiving is not implemented".to_owned(),
        ))
    }

    pub fn cancel(&mut self) -> Result<(), ChannelError> {
        Err(ChannelError::Unsupported(
            "client task cancellation is not implemented".to_owned(),
        ))
    }
}
