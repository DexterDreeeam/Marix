use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::{AsyncReceiver, AsyncSender, WorkQueue, build_async_channel};
use marix_protocol::{
    InvocationSignature, StepEvent, StepSignature, StepStatus,
};

use crate::invocation::Invocation;
use crate::task::TaskAccess;

pub struct StepState {
    pub access: Arc<TaskAccess>,
    pub signature: StepSignature,
    pub invocations: Arc<WorkQueue<InvocationSignature, Invocation>>,
    pub status: StdMutex<StepStatus>,
    pub step_tx: AsyncSender<StepEvent>,
    pub step_rx: StdMutex<Option<AsyncReceiver<StepEvent>>>,
}

// -- Private -- //

impl StepState {
    pub(crate) fn new(
        access: Arc<TaskAccess>,
        signature: StepSignature,
        invocations: Arc<WorkQueue<InvocationSignature, Invocation>>,
    ) -> Self {
        let (step_tx, step_rx) = build_async_channel();
        Self {
            access,
            signature,
            invocations,
            status: StdMutex::new(StepStatus::Created),
            step_tx,
            step_rx: StdMutex::new(Some(step_rx)),
        }
    }
}
