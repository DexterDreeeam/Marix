use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::AtomicUsize;

use marix_common::{Sender, WorkQueue};
use marix_protocol::{SessionEvent, TaskSignature};

use crate::model::ModelBackend;
use crate::plan::PlanQueue;
use crate::session::SessionContext;
use crate::step::Step;

pub struct TaskState {
    pub session_context: Arc<StdMutex<SessionContext>>,
    pub signature: TaskSignature,
    pub user_request: String,
    pub model_backend: StdMutex<Box<dyn ModelBackend>>,
    pub session_tx: Sender<SessionEvent>,
    pub step_count: AtomicUsize,
    pub plan_queue: PlanQueue,
    pub steps: WorkQueue<usize, Step>,
}

impl TaskState {
    pub fn new(
        session_context: Arc<StdMutex<SessionContext>>,
        signature: TaskSignature,
        user_request: String,
        model_backend: Box<dyn ModelBackend>,
        session_tx: Sender<SessionEvent>,
    ) -> Self {
        Self {
            session_context,
            signature,
            user_request,
            model_backend: StdMutex::new(model_backend),
            session_tx,
            step_count: AtomicUsize::new(0),
            plan_queue: PlanQueue::new(),
            steps: WorkQueue::new(),
        }
    }
}
