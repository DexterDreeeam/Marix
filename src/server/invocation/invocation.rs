use std::sync::Arc;

use marix_common::Logger;
use marix_protocol::{InvocationEvent, InvocationRequest, InvocationStatus};

use super::{InvocationRuntime, InvocationState};
use crate::task::TaskAccess;

#[derive(Clone)]
pub struct Invocation {
    pub access: Arc<TaskAccess>,
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

    pub fn result(&self) -> Option<String> {
        if !matches!(self.status(), InvocationStatus::Succeed { .. }) {
            return None;
        }
        let count = self
            .state
            .final_signal
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_ref()
            .copied()?;
        let output = self
            .state
            .output
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if (0..count).any(|seq| !output.contains_key(&seq)) {
            return None;
        }
        Some(
            (0..count)
                .filter_map(|seq| output.get(&seq))
                .cloned()
                .collect(),
        )
    }

    pub fn start(&self) {
        let runtime = InvocationRuntime::new(Arc::clone(&self.access), Arc::clone(&self.state));
        drop(self.access.rt.spawn(async move {
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
            Arc::clone(&access),
            request.signature,
            request.input,
        ));
        Self { access, state }
    }
}
