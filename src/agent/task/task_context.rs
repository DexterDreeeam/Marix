use std::sync::Mutex as StdMutex;

use marix_common::{ExeId, SessionEvent, SharedNetSender, TaskSignature, WorkQueue};

use crate::model::ModelBackend;
use crate::task::{Execution, Step, StepSequence};

pub struct TaskContext {
    pub signature: TaskSignature,
    pub model_backend: StdMutex<Box<dyn ModelBackend>>,
    pub client_tx: SharedNetSender<SessionEvent>,
    pub host_tx: SharedNetSender<SessionEvent>,
    pub executions: WorkQueue<ExeId, Execution>,
    pub steps: WorkQueue<StepSequence, Step>,
}

impl TaskContext {
    pub fn new(
        signature: TaskSignature,
        model_backend: Box<dyn ModelBackend>,
        client_tx: SharedNetSender<SessionEvent>,
        host_tx: SharedNetSender<SessionEvent>,
    ) -> Self {
        Self {
            signature,
            model_backend: StdMutex::new(model_backend),
            client_tx,
            host_tx,
            executions: WorkQueue::new(),
            steps: WorkQueue::new(),
        }
    }
}
