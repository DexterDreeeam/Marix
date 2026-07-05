use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::{Sender, WorkQueue};
use marix_protocol::{SessionEvent, TaskSignature};

use crate::model::ModelBackend;
use crate::session::SessionContext;
use crate::task::Step;

pub struct TaskState {
    pub session_context: Arc<StdMutex<SessionContext>>,
    pub signature: TaskSignature,
    pub model_backend: StdMutex<Box<dyn ModelBackend>>,
    pub session_tx: Sender<SessionEvent>,
    pub steps: WorkQueue<usize, Step>,
}

impl TaskState {
    pub fn new(
        session_context: Arc<StdMutex<SessionContext>>,
        signature: TaskSignature,
        model_backend: Box<dyn ModelBackend>,
        session_tx: Sender<SessionEvent>,
    ) -> Self {
        Self {
            session_context,
            signature,
            model_backend: StdMutex::new(model_backend),
            session_tx,
            steps: WorkQueue::new(),
        }
    }
}
