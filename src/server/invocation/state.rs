use std::collections::BTreeMap;
use std::sync::Mutex as StdMutex;

use marix_common::{AsyncReceiver, AsyncSender, build_async_channel};
use marix_common::external::*;
use marix_protocol::{
    ExecutionSignature, InvocationEvent, InvocationRequest, InvocationSignature, ToolInputSchema,
};

use crate::task::TaskAccess;

pub(super) struct InvocationState {
    pub(super) access: TaskAccess,
    pub(super) signature: InvocationSignature,
    pub(super) invocation_tx: AsyncSender<InvocationEvent>,
    pub(super) invocation_rx: StdMutex<Option<AsyncReceiver<InvocationEvent>>>,
    pub(super) input: ToolInputSchema,
    pub(super) execution_signature: StdMutex<Option<ExecutionSignature>>,
    pub(super) output: StdMutex<BTreeMap<usize, String>>,
    pub(super) final_signal: StdMutex<Option<usize>>,
}

impl InvocationState {
    pub(super) fn new(
        access: TaskAccess,
        signature: InvocationSignature,
        request: InvocationRequest,
    ) -> Self {
        let (invocation_tx, invocation_rx) = build_async_channel();
        Self {
            access,
            signature,
            invocation_tx,
            invocation_rx: StdMutex::new(Some(invocation_rx)),
            input: request.input,
            execution_signature: StdMutex::new(None),
            output: StdMutex::new(BTreeMap::new()),
            final_signal: StdMutex::new(None),
        }
    }
}
