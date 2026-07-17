use std::sync::Arc;

use marix_common::Actor;
use marix_protocol::{PlanDraft, PlanSignature, StepDraft, StepSignature};

use super::IntentRuntime;
use crate::intent::Intent;
use crate::plan::Plan;
use crate::step::Step;

impl IntentRuntime {
    pub(super) fn create_step(&self, draft: StepDraft) -> Result<(), String> {
        if self
            .plan
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .is_some()
        {
            return Err("intent cannot create a direct step after creating a plan".to_owned());
        }
        if draft.invocations.is_empty() {
            return Err("intent verdict step must contain an invocation".to_owned());
        }
        let signature = StepSignature::new(
            self.signature.clone(),
            format!("step-{}", self.steps.size() + 1),
        );
        let step = Step::from_draft(Arc::clone(&self.access), signature.clone(), draft)?;
        if !self.access.insert(step.clone()) {
            return Err(format!("step {signature} is duplicated"));
        }
        self.steps.insert(signature, None);
        step.start();
        Ok(())
    }

    pub(super) fn create_plan(&self, draft: PlanDraft) -> Result<(), String> {
        if draft.intents.is_empty() {
            return Err("intent verdict plan must contain a child intent".to_owned());
        }
        if self
            .plan
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .is_some()
        {
            return Err("intent already owns a plan".to_owned());
        }
        for (index, draft) in draft.intents.iter().enumerate() {
            if draft.content.trim().is_empty() {
                return Err(format!("plan child intent {} has empty content", index + 1));
            }
        }
        let plan_signature = PlanSignature::new(self.signature.clone(), "plan".to_owned());
        let mut intents = Vec::with_capacity(draft.intents.len());
        for (index, draft) in draft.intents.into_iter().enumerate() {
            let signature = marix_protocol::IntentSignature::new(
                self.access.signature.clone(),
                Some(plan_signature.clone()),
                format!("intent-{}", index + 1),
            );
            let intent = Intent::new(Arc::clone(&self.access), signature.clone(), draft.content);
            if !self.access.insert(intent.clone()) {
                return Err(format!("plan child intent {signature} is duplicated"));
            }
            intents.push(signature);
        }
        let plan = Plan::new(Arc::clone(&self.access), plan_signature.clone(), intents);
        if !self.access.insert(plan.clone()) {
            return Err(format!("plan {plan_signature} is duplicated"));
        }
        *self.plan.lock().unwrap_or_else(|error| error.into_inner()) = Some(plan_signature);
        plan.start();
        Ok(())
    }
}
