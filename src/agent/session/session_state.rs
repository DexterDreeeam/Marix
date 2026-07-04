use std::sync::{Arc, Mutex as StdMutex};

use marix_common::{SessionMessage, SharedNetReceiver, SharedNetSender, TaskId, WorkQueue};

use crate::task::Task;

pub struct SessionState {
    pub tasks: WorkQueue<TaskId, Arc<StdMutex<Task>>>,
    pub client_tx: SharedNetSender<SessionMessage>,
    pub client_rx: SharedNetReceiver<SessionMessage>,
    pub host_tx: SharedNetSender<SessionMessage>,
    pub host_rx: SharedNetReceiver<SessionMessage>,
}

impl SessionState {
    pub fn new() -> Self {
        Self {
            tasks: WorkQueue::new(),
            client_tx: SharedNetSender::new(std::sync::Mutex::new(None)),
            client_rx: SharedNetReceiver::new(std::sync::Mutex::new(None)),
            host_tx: SharedNetSender::new(std::sync::Mutex::new(None)),
            host_rx: SharedNetReceiver::new(std::sync::Mutex::new(None)),
        }
    }
}
