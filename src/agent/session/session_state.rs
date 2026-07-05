use std::sync::{Arc, Mutex as StdMutex};

use marix_common::{
    Receiver, Sender, SharedNetReceiver, SharedNetSender, System, WorkQueue, build_channel,
};
use marix_protocol::{SessionEvent, SessionMessage, TaskId};

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
    pub task_tx: Sender<SessionEvent>,
    pub task_rx: StdMutex<Receiver<SessionEvent>>,
}

impl SessionState {
    pub fn new() -> Self {
        let (task_tx, task_rx) = build_channel();
        Self {
            context: Arc::new(StdMutex::new(SessionContext {
                system: None,
                tasks: Vec::new(),
                tools: Vec::new(),
            })),
            host_sys: StdMutex::new(None),
            tasks: WorkQueue::new(),
            client_tx: SharedNetSender::new(std::sync::Mutex::new(None)),
            client_rx: SharedNetReceiver::new(std::sync::Mutex::new(None)),
            host_tx: SharedNetSender::new(std::sync::Mutex::new(None)),
            host_rx: SharedNetReceiver::new(std::sync::Mutex::new(None)),
            task_tx,
            task_rx: StdMutex::new(task_rx),
        }
    }
}
