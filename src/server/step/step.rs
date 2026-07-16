use std::sync::Arc;

use marix_common::Logger;
use marix_protocol::{
    InvocationRequest, InvocationSignature, StepDraft, StepEvent, StepResult, StepSignature,
    StepStatus,
};

use super::{StepRuntime, StepState};
use crate::invocation::Invocation;
use crate::task::TaskAccess;

#[derive(Clone)]
pub struct Step {
    pub state: Arc<StepState>,
}

impl Step {
    pub fn status(&self) -> StepStatus {
        self.state
            .status
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    pub fn result(&self) -> Option<StepResult> {
        match self.status() {
            StepStatus::Complete(result) => Some(result),
            StepStatus::Created | StepStatus::Running => None,
        }
    }

    pub fn start(&self) {
        let runtime = StepRuntime::new(Arc::clone(&self.state));
        drop(self.state.access.rt.spawn(async move {
            runtime.run().await;
        }));
    }

    pub fn dispatch(&self, event: StepEvent) {
        if self.state.step_tx.send(event).is_err() {
            Logger::warning(format!(
                "step {} event dispatch failed: worker stopped",
                &self.state.signature,
            ));
        }
    }
}

// -- Private -- //

impl Step {
    pub(crate) fn from_draft(
        access: Arc<TaskAccess>,
        signature: StepSignature,
        draft: StepDraft,
    ) -> Result<Self, String> {
        if draft.invocations.is_empty() {
            return Err("step must contain an invocation".to_owned());
        }
        if draft
            .invocations
            .iter()
            .any(|invocation| invocation.name.trim().is_empty())
        {
            return Err("step invocation name cannot be empty".to_owned());
        }
        let mut invocations = Vec::with_capacity(draft.invocations.len());
        for invocation in draft.invocations {
            let invocation_signature = InvocationSignature::new(
                signature.clone(),
                invocation.name,
            );
            let request = InvocationRequest {
                signature: invocation_signature.clone(),
                input: invocation.input,
            };
            let actor = Invocation::new(Arc::clone(&access), request);
            if !access.insert_invocation(actor.clone()) {
                return Err(format!(
                    "invocation {invocation_signature} is duplicated"
                ));
            }
            invocations.push(invocation_signature);
        }
        let state = Arc::new(StepState::new(access, signature, invocations));
        Ok(Self { state })
    }
}
