use crate::common::channel::ChannelError;
use crate::common::message::UserMessage;

use super::ClientTask;

pub struct ClientSession;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientSessionState {
    Disconnected,
    Connected,
    Closed,
}

impl ClientSession {
    pub fn connect() -> Result<Self, ChannelError> {
        panic!("not implemented")
    }

    pub fn state(&self) -> ClientSessionState {
        panic!("not implemented")
    }

    pub fn create_task(&mut self, _message: impl UserMessage) -> Result<ClientTask, ChannelError> {
        panic!("not implemented")
    }

    pub fn close(&mut self) -> Result<(), ChannelError> {
        panic!("not implemented")
    }
}
