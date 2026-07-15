use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::{AsyncReceiver, AsyncSender, build_async_channel};
use marix_protocol::{
    IntentSignature, PlanEvent, PlanResult, PlanSignature, PlanStatus,
};

use crate::task::TaskAccess;

pub struct PlanState {
    pub access: Arc<TaskAccess>,
    pub signature: PlanSignature,
    pub intents: StdMutex<Vec<IntentSignature>>,
    pub failures: StdMutex<Vec<PlanResult>>,
    pub status: StdMutex<PlanStatus>,
    pub plan_tx: AsyncSender<PlanEvent>,
    pub plan_rx: StdMutex<Option<AsyncReceiver<PlanEvent>>>,
}

// -- Private -- //

impl PlanState {
    pub(crate) fn new(
        access: Arc<TaskAccess>,
        signature: PlanSignature,
        intents: Vec<IntentSignature>,
    ) -> Self {
        let (plan_tx, plan_rx) = build_async_channel();
        Self {
            access,
            signature,
            intents: StdMutex::new(intents),
            failures: StdMutex::new(Vec::new()),
            status: StdMutex::new(PlanStatus::Created),
            plan_tx,
            plan_rx: StdMutex::new(Some(plan_rx)),
        }
    }
}
