use std::sync::Arc;

use marix_common::Logger;
use marix_protocol::{
    InvocationEvent, InvocationRequest, InvocationResult, InvocationStatus,
};

use super::{InvocationRuntime, InvocationState};
use crate::task::TaskAccess;

#[derive(Clone)]
pub struct Invocation {
    pub state: Arc<InvocationState>,
}

impl Invocation {
    pub fn status(&self) -> InvocationStatus {
        self.state
            .status
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    pub fn result(&self) -> Option<InvocationResult> {
        match self.status() {
            InvocationStatus::Complete(result) => Some(result),
            InvocationStatus::Created | InvocationStatus::Running => None,
        }
    }

    pub fn start(&self) {
        let runtime = InvocationRuntime::new(Arc::clone(&self.state));
        drop(self.state.access.rt.spawn(async move {
            runtime.run().await;
        }));
    }

    pub fn dispatch(&self, event: InvocationEvent) {
        if self.state.invocation_tx.send(event).is_err() {
            Logger::warning(format!(
                "invocation {} event dispatch failed: worker stopped",
                &self.state.signature,
            ));
        }
    }
}

// -- Private -- //

impl Invocation {
    pub(crate) fn new(access: Arc<TaskAccess>, request: InvocationRequest) -> Self {
        let state = Arc::new(InvocationState::new(
            access,
            request.signature,
            request.input,
        ));
        Self { state }
    }
}
