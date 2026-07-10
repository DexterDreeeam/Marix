use std::fmt;
use std::sync::Arc;

use marix_common::Logger;
use marix_protocol::{
    Actor, PlanError, PlanSignature, RuntimeAsync, StepDraft, StepEvent, StepKind, StepSignature,
};

use super::helper::step_kind;
use super::runtime::StepRuntime;
use super::state::StepState;
use crate::task::TaskAccess;

pub struct Step {
    state: Arc<StepState>,
}

impl Clone for Step {
    fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
        }
    }
}

impl Step {
    pub fn new(
        access: TaskAccess,
        signature: StepSignature,
        description: String,
        kind: StepKind,
    ) -> Self {
        let step = Self {
            state: Arc::new(StepState::new(signature, description, kind, access)),
        };
        step
    }

    pub(crate) fn from_draft(
        access: TaskAccess,
        plan: &PlanSignature,
        draft: StepDraft,
    ) -> Result<Self, PlanError> {
        let signature =
            StepSignature::new(access.signature.clone(), plan.clone(), draft.name.clone());
        let kind = step_kind(&signature, &draft)?;
        Ok(Self::new(access, signature, draft.description, kind))
    }

    pub(crate) fn signature(&self) -> &StepSignature {
        &self.state.signature
    }

    pub(crate) fn description(&self) -> &str {
        &self.state.description
    }

    pub(crate) fn kind(&self) -> &StepKind {
        &self.state.kind
    }
}

impl Actor<Step, StepEvent> for Step {
    fn start(&mut self) {
        let runtime = StepRuntime::new(Arc::clone(&self.state));
        drop(self.state.access.rt.spawn(async move {
            runtime.run().await;
        }));
    }

    fn dispatch(&self, event: StepEvent) {
        if self.state.step_tx.send(event).is_err() {
            Logger::warning(format!(
                "step {} event dispatch failed: worker stopped (task {})",
                &self.state.signature, &self.state.signature.task,
            ));
        }
    }
}

impl fmt::Debug for Step {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Step")
            .field("signature", &self.state.signature)
            .field("description", &self.state.description)
            .field("kind", &self.state.kind)
            .finish_non_exhaustive()
    }
}
