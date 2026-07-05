use std::sync::{Arc, Mutex as StdMutex};

use marix_common::{SharedNetReceiver, SharedNetSender, System, WorkQueue};
use marix_protocol::{SessionMessage, TaskId};

use crate::session::SessionContext;
use crate::task::Task;

pub struct SessionState {
    pub context: Arc<StdMutex<SessionContext>>,
    pub host_sys: StdMutex<Option<System>>,
    pub tasks: WorkQueue<TaskId, Arc<StdMutex<Task>>>,
    pub client_tx: SharedNetSender<SessionMessage>,
    pub client_rx: SharedNetReceiver<SessionMessage>,
    pub host_tx: SharedNetSender<SessionMessage>,
    pub host_rx: SharedNetReceiver<SessionMessage>,
}

impl SessionState {
    pub fn new() -> Self {
        Self {
            context: Arc::new(StdMutex::new(SessionContext {
                tasks: Vec::new(),
                tools: Vec::new(),
            })),
            host_sys: StdMutex::new(None),
            tasks: WorkQueue::new(),
            client_tx: SharedNetSender::new(std::sync::Mutex::new(None)),
            client_rx: SharedNetReceiver::new(std::sync::Mutex::new(None)),
            host_tx: SharedNetSender::new(std::sync::Mutex::new(None)),
            host_rx: SharedNetReceiver::new(std::sync::Mutex::new(None)),
        }
    }
}
