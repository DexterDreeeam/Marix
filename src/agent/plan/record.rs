use std::sync::atomic::{AtomicUsize, Ordering};

use marix_protocol::{Plan, PlanSignature, StepSignature};

pub struct PlanRecord {
    pub signature: PlanSignature,
    pub plan: Plan,
    pub step_signatures: Vec<StepSignature>,
    pub remaining_steps: AtomicUsize,
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
