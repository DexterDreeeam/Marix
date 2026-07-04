use std::sync::Mutex as StdMutex;

use marix_common::{SessionMessage, SharedNetSender, TaskSignature, WorkQueue};

use crate::model::ModelBackend;
use crate::task::Step;

pub struct TaskState {
    pub signature: TaskSignature,
    pub model_backend: StdMutex<Box<dyn ModelBackend>>,
    pub client_tx: SharedNetSender<SessionMessage>,
    pub host_tx: SharedNetSender<SessionMessage>,
    pub steps: WorkQueue<usize, Step>,
}

impl TaskState {
    pub fn new(
        signature: TaskSignature,
        model_backend: Box<dyn ModelBackend>,
        client_tx: SharedNetSender<SessionMessage>,
        host_tx: SharedNetSender<SessionMessage>,
    ) -> Self {
        Self {
            signature,
            model_backend: StdMutex::new(model_backend),
            client_tx,
            host_tx,
            steps: WorkQueue::new(),
        }
    }
}
