use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::{AsyncReceiver, AsyncSender, WorkQueue, build_async_channel};
use marix_protocol::{
    IntentEvent, IntentSignature, IntentStatus, PlanSignature,
    StepResult, StepSignature,
};

use crate::task::TaskAccess;

pub struct IntentState {
    pub access: Arc<TaskAccess>,
    pub signature: IntentSignature,
    pub content: String,
    pub steps: Arc<WorkQueue<StepSignature, Option<StepResult>>>,
    pub plan: StdMutex<Option<PlanSignature>>,
    pub status: StdMutex<IntentStatus>,
    pub intent_tx: AsyncSender<IntentEvent>,
    pub intent_rx: StdMutex<Option<AsyncReceiver<IntentEvent>>>,
}

// -- Private -- //

impl IntentState {
    pub(crate) fn new(
        access: Arc<TaskAccess>,
        signature: IntentSignature,
        content: String,
    ) -> Self {
        let (intent_tx, intent_rx) = build_async_channel();
        Self {
            access,
            signature,
            content,
            steps: Arc::new(WorkQueue::new()),
            plan: StdMutex::new(None),
            status: StdMutex::new(IntentStatus::Created),
            intent_tx,
            intent_rx: StdMutex::new(Some(intent_rx)),
        }
    }
}
