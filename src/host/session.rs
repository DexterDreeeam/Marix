use std::thread::JoinHandle;

use marix_common::{Receiver, Sender, SessionEvent};

use crate::executor::Executor;

pub struct HostSession {
    agent_tx: Sender<SessionEvent>,
    agent_rx: Receiver<SessionEvent>,
    executor: Executor,
    event_loop: Option<JoinHandle<()>>,
}

impl HostSession {
    pub fn connect(agent_tx: Sender<SessionEvent>, agent_rx: Receiver<SessionEvent>) -> Self {
        panic!("not implemented")
    }

    pub fn close(&mut self) {
        panic!("not implemented")
    }
}
