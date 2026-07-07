use std::fmt;
use std::sync::Arc;

use marix_protocol::{PlanDraft, PlanSignature, StepDraft, StepSignature};

use crate::plan::PlanError;
use crate::step::Step;
use crate::task::TaskState;

#[derive(Clone)]
pub struct Plan {
    pub signature: PlanSignature,
    pub description: String,
    pub run_steps: Vec<Step>,
    pub pending_steps: Vec<Step>,
    pub expected_result: String,
}

impl Plan {
    pub(crate) fn from_draft(
        state: &Arc<TaskState>,
        signature: PlanSignature,
        draft: PlanDraft,
    ) -> Result<Self, PlanError> {
        let run_steps = Self::build_steps(state, draft.run_steps)?;
        let pending_steps = Self::build_steps(state, draft.pending_steps)?;
        Ok(Self {
            signature,
            description: draft.description,
            run_steps,
            pending_steps,
            expected_result: draft.expected_result,
        })
    }

    pub(crate) fn run_step_signatures(&self) -> Vec<StepSignature> {
        self.run_steps
            .iter()
            .map(|step| step.signature.clone())
            .collect()
    }
}

// -- Private -- //

impl Plan {
    fn build_steps(state: &Arc<TaskState>, drafts: Vec<StepDraft>) -> Result<Vec<Step>, PlanError> {
        drafts
            .into_iter()
            .map(|draft| Step::from_draft(state, draft))
            .collect()
    }
}

impl fmt::Debug for Plan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let run_steps = self
            .run_steps
            .iter()
            .map(|step| &step.signature)
            .collect::<Vec<_>>();
        let pending_steps = self
            .pending_steps
            .iter()
            .map(|step| &step.signature)
            .collect::<Vec<_>>();
        formatter
            .debug_struct("Plan")
            .field("signature", &self.signature)
            .field("description", &self.description)
            .field("run_steps", &run_steps)
            .field("pending_steps", &pending_steps)
            .field("expected_result", &self.expected_result)
            .finish()
    }
}
