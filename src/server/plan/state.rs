use std::sync::Mutex as StdMutex;
use std::sync::atomic::AtomicBool;

use marix_common::{AsyncReceiver, AsyncSender, build_async_channel};
use marix_protocol::{PlanEvent, PlanSignature};

use crate::step::Step;
use crate::task::TaskAccess;

pub(crate) struct PlanState {
    pub(crate) access: TaskAccess,
    pub(crate) signature: PlanSignature,
    pub(crate) description: String,
    pub(crate) background: String,
    pub(crate) call: Vec<Step>,
    pub(crate) model: Step,
    pub(crate) model_once: AtomicBool,
    pub(crate) future: Vec<Step>,
    pub(crate) expected_result: String,
    pub(crate) plan_tx: AsyncSender<PlanEvent>,
    pub(crate) plan_rx: StdMutex<Option<AsyncReceiver<PlanEvent>>>,
}

impl PlanState {
    pub(super) fn new(
        access: TaskAccess,
        signature: PlanSignature,
        description: String,
        background: String,
        call: Vec<Step>,
        model: Step,
        future: Vec<Step>,
        expected_result: String,
    ) -> Self {
        let (plan_tx, plan_rx) = build_async_channel();
        Self {
            access,
            signature,
            description,
            background,
            call,
            model,
            model_once: AtomicBool::new(false),
            future,
            expected_result,
            plan_tx,
            plan_rx: StdMutex::new(Some(plan_rx)),
        }
    }
}
