use crate::common::channel::ChannelError;

use super::AgentTask;

pub struct AgentSession;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentSessionState {
    WaitingClient,
    Connected,
    WaitingTask,
    Closed,
}

impl AgentSession {
    pub fn new() -> Self {
        panic!("not implemented")
    }

    pub fn state(&self) -> AgentSessionState {
        panic!("not implemented")
    }

    pub fn accept(&mut self) -> Result<(), ChannelError> {
        panic!("not implemented")
    }

    pub async fn accept_task(&self) -> Result<AgentTask, ChannelError> {
        panic!("not implemented")
    }

    pub fn close(&mut self) -> Result<(), ChannelError> {
        panic!("not implemented")
    }
}
