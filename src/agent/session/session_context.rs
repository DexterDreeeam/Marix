use std::sync::{Arc, Mutex as StdMutex};

use crate::task::Task;
use marix_common::{SessionEvent, SharedNetReceiver, SharedNetSender, TaskId, WorkQueue};

pub struct SessionContext {
    pub tasks: WorkQueue<TaskId, Arc<StdMutex<Task>>>,
    pub client_tx: SharedNetSender<SessionEvent>,
    pub client_rx: SharedNetReceiver<SessionEvent>,
    pub host_tx: SharedNetSender<SessionEvent>,
    pub host_rx: SharedNetReceiver<SessionEvent>,
}

impl SessionContext {
    pub fn new() -> Self {
        Self {
            tasks: WorkQueue::new(),
            client_tx: SharedNetSender::new(tokio::sync::Mutex::new(None)),
            client_rx: SharedNetReceiver::new(tokio::sync::Mutex::new(None)),
            host_tx: SharedNetSender::new(tokio::sync::Mutex::new(None)),
            host_rx: SharedNetReceiver::new(tokio::sync::Mutex::new(None)),
        }
    }
}
