use std::sync::Arc;

use marix_common::Logger;
use marix_protocol::{IntentEvent, IntentResult, IntentSignature, IntentStatus};

use super::{IntentRuntime, IntentState};
use crate::task::TaskAccess;

#[derive(Clone)]
pub struct Intent {
    pub state: Arc<IntentState>,
}

impl Intent {
    pub fn status(&self) -> IntentStatus {
        self.state
            .status
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    pub fn result(&self) -> Option<IntentResult> {
        match self.status() {
            IntentStatus::Complete(result) => Some(result),
            IntentStatus::Created | IntentStatus::Running => None,
        }
    }

    pub fn start(&self) {
        let runtime = IntentRuntime::new(Arc::clone(&self.state));
        drop(self.state.access.rt.spawn(async move {
            runtime.run().await;
        }));
    }

    pub fn dispatch(&self, event: IntentEvent) {
        if self.state.intent_tx.send(event).is_err() {
            Logger::warning(format!(
                "intent {} event dispatch failed: worker stopped",
                &self.state.signature,
            ));
        }
    }
}

// -- Private -- //

impl Intent {
    pub(crate) fn new(
        access: Arc<TaskAccess>,
        signature: IntentSignature,
        content: String,
    ) -> Self {
        let state = Arc::new(IntentState::new(access, signature, content));
        Self { state }
    }
}
