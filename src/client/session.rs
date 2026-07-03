use std::thread::JoinHandle;

use marix_common::{Receiver, Sender, SessionEvent, TaskId};

use crate::ClientEvent;

pub struct ClientSession {
    agent_tx: Sender<SessionEvent>,
    agent_rx: Receiver<SessionEvent>,
    user_tx: Sender<ClientEvent>,
    pub user_rx: Receiver<ClientEvent>,
    event_loop: Option<JoinHandle<()>>,
}

impl ClientSession {
    pub fn connect(agent_tx: Sender<SessionEvent>, agent_rx: Receiver<SessionEvent>) -> Self {
        panic!("not implemented")
    }

    pub fn create_task(&self, request: String) {
        panic!("not implemented")
    }

    pub fn query_task(&self, task_id: TaskId) {
        panic!("not implemented")
    }

    pub fn cancel_task(&self, task_id: TaskId) {
        panic!("not implemented")
    }

    pub fn close(&mut self) {
        panic!("not implemented")
    }
}
