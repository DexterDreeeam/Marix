use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::AtomicUsize;

use marix_common::{Sender, WorkQueue};
use marix_protocol::{Plan, SessionEvent, StepSignature, TaskSignature};

use crate::model::ModelBackend;
use crate::session::SessionContext;
use crate::step::Step;

pub struct TaskState {
    pub session_context: Arc<StdMutex<SessionContext>>,
    pub signature: TaskSignature,
    pub model_backend: StdMutex<Box<dyn ModelBackend>>,
    pub session_tx: Sender<SessionEvent>,
    pub step_count: AtomicUsize,
    pub plan_list: StdMutex<Vec<(Plan, Vec<StepSignature>)>>,
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
            step_count: AtomicUsize::new(0),
            plan_list: StdMutex::new(Vec::new()),
            steps: WorkQueue::new(),
        }
    }
}
