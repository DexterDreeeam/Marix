use std::sync::{Arc, Mutex as StdMutex};

use marix_common::{
    Receiver, Sender, SharedNetReceiver, SharedNetSender, System, WorkQueue, build_channel,
    external::uuid,
};
use marix_protocol::{SessionEvent, SessionMessage};

use crate::session::SessionContext;

pub struct SessionState {
    pub session_id: uuid::Uuid,
    pub context: Arc<StdMutex<SessionContext>>,
    pub host_sys: StdMutex<Option<System>>,
    pub client_tx: SharedNetSender<SessionMessage>,
    pub client_rx: SharedNetReceiver<SessionMessage>,
    pub host_tx: SharedNetSender<SessionMessage>,
    pub host_rx: SharedNetReceiver<SessionMessage>,
    pub session_tx: Sender<SessionEvent>,
    pub session_rx: Receiver<SessionEvent>,
}

impl SessionState {
    pub fn new() -> Self {
        let (session_tx, session_rx) = build_channel();
        Self {
            session_id: uuid::Uuid::new_v4(),
            context: Arc::new(StdMutex::new(SessionContext {
                system: None,
                tasks: WorkQueue::new(),
                tools: Vec::new(),
            })),
            host_sys: StdMutex::new(None),
            client_tx: SharedNetSender::new(std::sync::Mutex::new(None)),
            client_rx: SharedNetReceiver::new(std::sync::Mutex::new(None)),
            host_tx: SharedNetSender::new(std::sync::Mutex::new(None)),
            host_rx: SharedNetReceiver::new(std::sync::Mutex::new(None)),
            session_tx,
            session_rx,
        }
    }
}
