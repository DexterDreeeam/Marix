use std::fmt;
use std::sync::Arc;

use marix_common::Logger;
use marix_protocol::{
    Actor, InvocationEvent, InvocationRequest, InvocationSignature, RuntimeAsync,
};

use super::runtime::InvocationRuntime;
use super::state::InvocationState;
use crate::task::TaskAccess;

pub struct Invocation {
    state: Arc<InvocationState>,
}

impl Clone for Invocation {
    fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
        }
    }
}

impl Invocation {
    pub fn new(access: TaskAccess, request: InvocationRequest) -> Self {
        let signature = request.signature.clone();
        let state = Arc::new(InvocationState::new(access, signature, request));
        Self { state }
    }

    pub(crate) fn signature(&self) -> &InvocationSignature {
        &self.state.signature
    }
}

impl Actor<Invocation, InvocationEvent> for Invocation {
    fn start(&mut self) {
        let runtime = InvocationRuntime::new(Arc::clone(&self.state));
        drop(self.state.access.rt.spawn(async move {
            runtime.run().await;
        }));
    }

    fn dispatch(&self, event: InvocationEvent) {
        if self.state.invocation_tx.send(event).is_err() {
            Logger::warning(format!(
                "invocation {} event dispatch failed: worker stopped",
                &self.state.signature,
            ));
        }
    }
}

impl fmt::Debug for Invocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Invocation")
            .field("signature", &self.state.signature)
            .field("step", &self.state.signature.step)
            .finish_non_exhaustive()
    }
}
