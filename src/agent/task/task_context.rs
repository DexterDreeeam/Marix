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
    pub runtime: tokio::runtime::Runtime,
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
            runtime: tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .unwrap_or_else(|error| panic!("failed to build task runtime: {error}")),
        }
    }
}
