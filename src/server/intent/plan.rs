use std::sync::Arc;

use marix_common::{ActorStatus, Logger, Runtime as RuntimeTrait};
use marix_protocol::{
    IntentEvent, IntentResult, IntentResultKind, IntentSignature, PlanDraft, PlanResult,
    SessionEvent, TaskEvent,
};

use super::IntentRuntime;
use crate::intent::Intent;
use crate::plan::Plan;

impl IntentRuntime {
    pub(super) fn create_plan(&self, draft: PlanDraft) -> Result<(), String> {
        self.validate_plan_draft(&draft)?;

        let mut current_plan = self.plan.lock().unwrap_or_else(|error| error.into_inner());
        if current_plan.is_some() {
            return Err("cannot create a plan while intent has an active plan".to_owned());
        }

        let plan = self.create_subintents(draft)?;
        let first = plan
            .subintents
            .first()
            .cloned()
            .ok_or_else(|| "intent verdict plan has no subintent".to_owned())?;
        *current_plan = Some(plan);
        drop(current_plan);
        self.start_subintent(first)
    }

    pub(super) fn on_subintent_update(
        &self,
        signature: IntentSignature,
        status: ActorStatus<IntentResult>,
    ) {
        if matches!(self.status(), ActorStatus::Complete(_)) {
            Logger::error(format!(
                "intent {} received subintent {signature} update \
                 {status:?} after completion",
                &self.signature,
            ));
            return;
        }
        let ActorStatus::Complete(result) = status else {
            return;
        };
        let plan = {
            let plan = self.plan.lock().unwrap_or_else(|error| error.into_inner());
            let Some(plan) = plan.as_ref() else {
                self.fail(format!(
                    "intent received subintent update from {signature} \
                     without an active plan",
                ));
                return;
            };
            plan.clone()
        };
        let Some(index) = plan
            .subintents
            .iter()
            .position(|candidate| candidate == &signature)
        else {
            self.fail(format!(
                "intent received update from unexpected subintent \
                 {signature}",
            ));
            return;
        };

        match result.kind {
            IntentResultKind::Succeed => {
                let Some(next) = plan.subintents.get(index + 1).cloned() else {
                    self.finish(IntentResultKind::Succeed, result.output);
                    return;
                };
                if let Err(reason) = self.start_subintent(next) {
                    self.fail(reason);
                }
            }
            IntentResultKind::Infeasible | IntentResultKind::Failed => {
                if let Err(reason) = self.record_failure() {
                    self.fail(reason);
                    return;
                }
                *self.plan.lock().unwrap_or_else(|error| error.into_inner()) = None;
                if let Err(reason) = self.verdict() {
                    self.fail(reason);
                }
            }
            IntentResultKind::Canceled => {
                self.finish(IntentResultKind::Canceled, result.output);
            }
        }
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
                TaskEvent::Intent(signature.clone(), IntentEvent::Cancel),
            );
            if self.access.session_tx.send(event).is_err() {
                Logger::warning(format!(
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
    fn validate_plan_draft(&self, draft: &PlanDraft) -> Result<(), String> {
        if draft.intents.is_empty() {
            return Err("intent verdict plan must contain a subintent".to_owned());
        }
        for (index, draft) in draft.intents.iter().enumerate() {
            if draft.content.trim().is_empty() {
                return Err(format!("plan subintent {} has empty content", index + 1,));
            }
        }
        Ok(())
    }

    fn record_failure(&self) -> Result<(), String> {
        let plan = self
            .plan
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
            .ok_or_else(|| "cannot record a failure without an active plan".to_owned())?;
        let goals = plan
            .subintents
            .iter()
            .map(|signature| {
                self.access.get_intent_content(signature).ok_or_else(|| {
                    format!(
                        "cannot snapshot plan: subintent {signature} \
                         was not found",
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let results = plan
            .subintents
            .iter()
            .map(|signature| self.access.get_result(signature))
            .collect::<Vec<_>>();
        let reason = results
            .iter()
            .flatten()
            .find_map(|result| match result.kind {
                IntentResultKind::Failed | IntentResultKind::Infeasible => {
                    Some(result.output.clone())
                }
                IntentResultKind::Succeed | IntentResultKind::Canceled => None,
            })
            .ok_or_else(|| {
                "cannot record plan failure: no subintent failed or was infeasible".to_owned()
            })?;
        self.plan_failures
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .push(PlanResult {
                goals,
                results,
                reason,
            });
        Ok(())
    }

    fn create_subintents(&self, draft: PlanDraft) -> Result<Plan, String> {
        let failure_count = self
            .plan_failures
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .len();
        let mut subintents = Vec::with_capacity(draft.intents.len());
        for (index, draft) in draft.intents.into_iter().enumerate() {
            let name = if failure_count == 0 {
                format!("subintent-{}", index + 1)
            } else {
                format!("subintent-r{failure_count}-{}", index + 1)
            };
            let signature = IntentSignature::new(
                self.access.signature.clone(),
                Some(self.signature.clone()),
                name,
            );
            let intent = Intent::new(Arc::clone(&self.access), signature.clone(), draft.content);
            if !self.access.insert(intent) {
                return Err(format!("plan subintent {signature} is duplicated"));
            }
            subintents.push(signature);
        }
        Ok(Plan { subintents })
    }

    fn start_subintent(&self, signature: IntentSignature) -> Result<(), String> {
        let event = SessionEvent::Task(
            self.access.signature.clone(),
            TaskEvent::IntentStart(signature.clone()),
        );
        self.access
            .session_tx
            .send(event)
            .map_err(|_| format!("plan subintent {signature} start failed: session stopped",))
    }
}
