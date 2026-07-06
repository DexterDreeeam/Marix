use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::Ordering;

use marix_common::Logger;
use marix_protocol::{Plan, PlanEvent, PlanSignature, StepEvent, StepSignature};

use crate::plan::{PlanError, PlanRecord, PlanStringify};
use crate::step::Step;
use crate::task::TaskState;

pub struct PlanHub {
    records: Mutex<Vec<PlanRecord>>,
}

impl PlanHub {
    pub fn new() -> Self {
        Self {
            records: Mutex::new(Vec::new()),
        }
    }

    pub(crate) fn route_event(
        &self,
        state: &Arc<TaskState>,
        signature: PlanSignature,
        event: PlanEvent,
    ) {
        match event {
            PlanEvent::Trigger(plan) => self.run_plan(state, signature, plan),
        }
    }

    pub(crate) fn run_plan(&self, state: &Arc<TaskState>, signature: PlanSignature, plan: Plan) {
        let _ = Logger::debug(format!(
            "running plan with {} step(s) (task {})",
            plan.run_steps.len(),
            state.signature.id.0
        ));
        // Reserve a unique, contiguous step-number block for this plan up front.
        let base_step_no = state
            .step_count
            .fetch_add(plan.run_steps.len(), Ordering::Relaxed);
        let step_signatures: Vec<StepSignature> = plan
            .run_steps
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, draft)| {
                StepSignature::new(
                    state.signature.clone(),
                    base_step_no + index,
                    draft.description,
                    draft.kind,
                )
            })
            .collect();
        self.insert(signature, plan, step_signatures.clone())
            .unwrap_or_else(|error| panic!("failed to insert task plan: {error:?}"));
        for step_signature in step_signatures {
            Step::send_step_event(state, &step_signature, StepEvent::Trigger);
        }
    }

    pub fn complete_step(&self, step_signature: &StepSignature) -> Option<PlanSignature> {
        let records = self
            .records
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        records.iter().find_map(|record| {
            record
                .complete_step(step_signature)
                .then(|| record.signature.clone())
        })
    }

    pub fn get(&self, signature: &PlanSignature) -> Result<Plan, PlanError> {
        self.records
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .iter()
            .find(|record| &record.signature == signature)
            .map(|record| record.plan.clone())
            .ok_or(PlanError::PlanNotFound)
    }

    pub fn insert(
        &self,
        signature: PlanSignature,
        plan: Plan,
        step_signatures: Vec<StepSignature>,
    ) -> Result<(), PlanError> {
        let mut records = self
            .records
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if records.iter().any(|record| record.signature == signature) {
            return Err(PlanError::DuplicatePlan);
        }
        records.push(PlanRecord::new(signature, plan, step_signatures));
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<PlanSignature>, PlanError> {
        Ok(self
            .records
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .iter()
            .map(|record| record.signature.clone())
            .collect())
    }

    pub fn stringify(&self) -> PlanStringify {
        let records = self
            .records
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        PlanStringify::new(records)
    }
}
