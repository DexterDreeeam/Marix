use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::{Sender, WorkQueue};
use marix_protocol::{ExecutionSignature, SessionEvent, StepSignature, TaskSignature};

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
    pub plan_queue: PlanQueue,
    pub execution_map: StdMutex<HashMap<ExecutionSignature, StepSignature>>,
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
            plan_queue: PlanQueue::new(),
            execution_map: StdMutex::new(HashMap::new()),
            steps: WorkQueue::new(),
        }
    }
}

// -- Private -- //

impl fmt::Debug for TaskState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TaskState")
            .field("signature", &self.signature)
            .finish_non_exhaustive()
    }
}
