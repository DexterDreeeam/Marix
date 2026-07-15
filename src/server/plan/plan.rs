use std::sync::Arc;

use marix_common::Logger;
use marix_protocol::{IntentSignature, PlanEvent, PlanResult, PlanSignature, PlanStatus};

use super::{PlanRuntime, PlanState};
use crate::task::TaskAccess;

#[derive(Clone)]
pub struct Plan {
    pub state: Arc<PlanState>,
}

impl Plan {
    pub fn status(&self) -> PlanStatus {
        self.state
            .status
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    pub fn result(&self) -> Option<PlanResult> {
        match self.status() {
            PlanStatus::Complete(result) => Some(result),
            PlanStatus::Created | PlanStatus::Running => None,
        }
    }

    pub fn start(&self) {
        let runtime = PlanRuntime::new(Arc::clone(&self.state));
        drop(self.state.access.rt.spawn(async move {
            runtime.run().await;
        }));
    }

    pub fn dispatch(&self, event: PlanEvent) {
        if self.state.plan_tx.send(event).is_err() {
            Logger::warning(format!(
                "plan {} event dispatch failed: worker stopped",
                &self.state.signature,
            ));
        }
    }
}

// -- Private -- //

impl Plan {
    pub(crate) fn new(
        access: Arc<TaskAccess>,
        signature: PlanSignature,
        intents: Vec<IntentSignature>,
    ) -> Self {
        let state = Arc::new(PlanState::new(access, signature, intents));
        Self { state }
    }
}
