use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::{SharedNetSender, WorkQueue};
use marix_protocol::{SessionMessage, TaskSignature};

use crate::model::ModelBackend;
use crate::session::SessionContext;
use crate::task::Step;

pub struct TaskState {
    pub session_context: Arc<StdMutex<SessionContext>>,
    pub signature: TaskSignature,
    pub model_backend: StdMutex<Box<dyn ModelBackend>>,
    pub client_tx: SharedNetSender<SessionMessage>,
    pub host_tx: SharedNetSender<SessionMessage>,
    pub steps: WorkQueue<usize, Step>,
}

impl TaskState {
    pub fn new(
        session_context: Arc<StdMutex<SessionContext>>,
        signature: TaskSignature,
        model_backend: Box<dyn ModelBackend>,
        client_tx: SharedNetSender<SessionMessage>,
        host_tx: SharedNetSender<SessionMessage>,
    ) -> Self {
        Self {
            session_context,
            signature,
            model_backend: StdMutex::new(model_backend),
            client_tx,
            host_tx,
            steps: WorkQueue::new(),
        }
    }
}
