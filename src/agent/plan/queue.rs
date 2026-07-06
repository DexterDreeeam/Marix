use std::sync::Mutex;

use marix_protocol::{Plan, PlanSignature, StepSignature};

use crate::plan::{PlanError, PlanRecord};

pub struct PlanQueue {
    records: Mutex<Vec<PlanRecord>>,
}

impl PlanQueue {
    pub fn new() -> Self {
        Self {
            records: Mutex::new(Vec::new()),
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

    pub fn current_plan_text(&self) -> String {
        self.records
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .iter()
            .map(|record| format!("{:?}", record.plan))
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn pending_intentions_text(&self) -> String {
        self.records
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .iter()
            .flat_map(|record| record.plan.pending_steps.iter())
            .map(|step| step.description.clone())
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub(crate) fn step_signatures(
        &self,
        signature: &PlanSignature,
    ) -> Result<Vec<StepSignature>, PlanError> {
        self.records
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .iter()
            .find(|record| &record.signature == signature)
            .map(|record| record.step_signatures.clone())
            .ok_or(PlanError::PlanNotFound)
    }
}
