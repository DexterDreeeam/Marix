use std::sync::Arc;

use marix_common::{ActorStatus, Runtime as RuntimeTrait};
use marix_protocol::{
    IntentEvent, IntentResult, IntentSignature, PlanDraft, SessionEvent,
    TaskEvent, TaskLogging,
};

use super::IntentRuntime;
use crate::intent::Intent;
use crate::plan::Plan;

impl IntentRuntime {
    pub(super) fn create_plan(
        &self,
        draft: PlanDraft,
    ) -> Result<(), String> {
        self.validate_plan_draft(&draft)?;
        let mut current_plan = self
            .plan
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if current_plan.is_some() {
            return Err(
                "cannot create a plan while intent has an active plan"
                    .to_owned(),
            );
        }
        let plan = self.create_subintents(draft)?;
        let first = plan
            .subintents
            .first()
            .cloned()
            .ok_or_else(|| {
                "intent stage plan has no subintent".to_owned()
            })?;
        *current_plan = Some(plan);
        drop(current_plan);
        self.access
            .session_tx
            .send(SessionEvent::Task(
                self.access.signature.clone(),
                TaskEvent::IntentStart(first.clone()),
            ))
            .map_err(|_| {
                format!(
                    "plan subintent {first} start failed: \
                     session stopped"
                )
            })
    }

    pub(super) fn cancel_plan(&self) {
        let plan = self
            .plan
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        let Some(plan) = plan else {
            return;
        };
        for signature in plan.subintents {
            if self.access.get_result(&signature).is_some() {
                continue;
            }
            let event = SessionEvent::Task(
                self.access.signature.clone(),
                TaskEvent::Intent(
                    signature.clone(),
                    IntentEvent::Cancel,
                ),
            );
            if self.access.session_tx.send(event).is_err() {
                self.warning(format!(
                    "intent {} subintent {signature} cancel failed: \
                     session stopped",
                    &self.signature,
                ));
            }
        }
    }
}

// -- Private -- //

impl IntentRuntime {
    fn validate_plan_draft(
        &self,
        draft: &PlanDraft,
    ) -> Result<(), String> {
        if draft.intents.is_empty() {
            return Err(
                "intent stage plan must contain a subintent".to_owned(),
            );
        }
        for (index, draft) in draft.intents.iter().enumerate() {
            if draft.content.trim().is_empty() {
                return Err(format!(
                    "plan subintent {} has empty content",
                    index + 1,
                ));
            }
        }
        Ok(())
    }

    fn create_subintents(
        &self,
        draft: PlanDraft,
    ) -> Result<Plan, String> {
        let failure_count = self
            .plan_failures
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .len();
        let mut subintents =
            Vec::with_capacity(draft.intents.len());
        for (index, draft) in
            draft.intents.into_iter().enumerate()
        {
            let name = if failure_count == 0 {
                format!("subintent-{}", index + 1)
            } else {
                format!(
                    "subintent-r{failure_count}-{}",
                    index + 1,
                )
            };
            let signature = IntentSignature::new(
                self.access.signature.clone(),
                Some(self.signature.clone()),
                name,
            );
            let intent = Intent::new(
                Arc::clone(&self.access),
                signature.clone(),
                draft.content,
            );
            if !self.access.insert(intent) {
                return Err(format!(
                    "plan subintent {signature} is duplicated"
                ));
            }
            subintents.push(signature);
        }
        Ok(Plan { subintents })
    }
}

#[allow(dead_code)]
fn assert_runtime_object_safe(
    runtime: &dyn RuntimeTrait<Base = Intent, Prepared = ()>,
) {
    let _ = runtime.run();
}
