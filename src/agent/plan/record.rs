use std::sync::atomic::{AtomicUsize, Ordering};

use marix_protocol::{PlanSignature, StepSignature};

use crate::plan::Plan;

pub struct PlanRecord {
    pub signature: PlanSignature,
    pub plan: Plan,
    pub step_signatures: Vec<StepSignature>,
    pub remaining_steps: AtomicUsize,
}

impl Clone for PlanRecord {
    fn clone(&self) -> Self {
        Self {
            signature: self.signature.clone(),
            plan: self.plan.clone(),
            step_signatures: self.step_signatures.clone(),
            remaining_steps: AtomicUsize::new(self.remaining_steps.load(Ordering::Relaxed)),
        }
    }
}

impl PlanRecord {
    pub fn new(signature: PlanSignature, plan: Plan, step_signatures: Vec<StepSignature>) -> Self {
        let remaining_steps = AtomicUsize::new(step_signatures.len());
        Self {
            signature,
            plan,
            step_signatures,
            remaining_steps,
        }
    }

    pub fn complete_step(&self, step_signature: &StepSignature) -> bool {
        if !self
            .step_signatures
            .iter()
            .any(|signature| signature == step_signature)
        {
            return false;
        }
        self.remaining_steps
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |remaining| {
                remaining.checked_sub(1)
            })
            .is_ok_and(|remaining| remaining == 1)
    }
}
