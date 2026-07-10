use std::sync::Mutex as StdMutex;

use marix_common::{AsyncReceiver, AsyncSender, build_async_channel};
use marix_common::external::*;
use marix_protocol::{StepEvent, StepKind, StepSignature};

use crate::task::TaskAccess;

pub(super) struct StepState {
    pub(super) signature: StepSignature,
    pub(super) description: String,
    pub(super) kind: StepKind,
    pub(super) access: TaskAccess,
    pub(super) step_tx: AsyncSender<StepEvent>,
    pub(super) step_rx: StdMutex<Option<AsyncReceiver<StepEvent>>>,
}

impl StepState {
    pub(super) fn new(
        signature: StepSignature,
        description: String,
        kind: StepKind,
        access: TaskAccess,
    ) -> Self {
        let (step_tx, step_rx) = build_async_channel();
        Self {
            signature,
            description,
            kind,
            access,
            step_tx,
            step_rx: StdMutex::new(Some(step_rx)),
        }
    }
}
