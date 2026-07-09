use std::sync::Mutex;

use marix_protocol::{PlanError, PlanSignature, StepSignature};

use crate::plan::{Plan, PlanRecord, PlanStringify};

pub struct PlanHub {
    records: Mutex<Vec<PlanRecord>>,
}

impl PlanHub {
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

    pub(crate) fn with_mut<R>(
        &self,
        signature: &PlanSignature,
        function: impl FnOnce(&mut Plan) -> R,
    ) -> Option<R> {
        self.records
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .iter_mut()
            .find(|record| &record.signature == signature)
            .map(|record| function(&mut record.plan))
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
