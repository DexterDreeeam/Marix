use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::{AsyncReceiver, AsyncSender, build_async_channel};
use marix_protocol::{
    ExecutionSignature, InvocationEvent, InvocationSignature, InvocationStatus, ToolInputSchema,
};

use crate::task::TaskAccess;

pub struct InvocationState {
    pub access: Arc<TaskAccess>,
    pub signature: InvocationSignature,
    pub input: ToolInputSchema,
    pub status: StdMutex<InvocationStatus>,
    pub output: StdMutex<BTreeMap<usize, String>>,
    pub final_signal: StdMutex<Option<usize>>,
    pub execution: StdMutex<Option<ExecutionSignature>>,
    pub invocation_tx: AsyncSender<InvocationEvent>,
    pub invocation_rx: StdMutex<Option<AsyncReceiver<InvocationEvent>>>,
}

// -- Private -- //

impl InvocationState {
    pub(crate) fn new(
        access: Arc<TaskAccess>,
        signature: InvocationSignature,
        input: ToolInputSchema,
    ) -> Self {
        let (invocation_tx, invocation_rx) = build_async_channel();
        Self {
            access,
            signature,
            input,
            status: StdMutex::new(InvocationStatus::Created),
            output: StdMutex::new(BTreeMap::new()),
            final_signal: StdMutex::new(None),
            execution: StdMutex::new(None),
            invocation_tx,
            invocation_rx: StdMutex::new(Some(invocation_rx)),
        }
    }
}
