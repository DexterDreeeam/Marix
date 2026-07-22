use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::{
    Actor, ActorStartFuture, ActorStatus, Lifecycle, Logger, Runtime as RuntimeTrait, WorkQueue,
};
use marix_protocol::{
    IntentEvent, IntentResult, IntentResultKind, IntentSignature, PlanResult, RelayKind,
    RelayRequest, RelayResult, RelayResultKind, RelaySignature, SessionEvent, StepDraft, StepEvent,
    StepResult, StepResultKind, StepSignature, TaskEvent, WorkflowCallSummary, WorkflowComplete,
    WorkflowInfeasible, WorkflowPlan, WorkflowTool,
};

use super::Intent;
use crate::plan::Plan;
use crate::relay::Relay;
use crate::step::Step;
use crate::task::TaskAccess;

pub struct IntentRuntime {
    pub access: Arc<TaskAccess>,
    pub signature: IntentSignature,
    pub content: String,
    pub steps: Arc<WorkQueue<StepSignature, Option<StepResult>>>,
    pub plan: StdMutex<Option<Plan>>,
    pub plan_failures: StdMutex<Vec<PlanResult>>,
    pub lifecycle: Lifecycle<IntentEvent, IntentResult>,
}

impl IntentRuntime {
    pub(crate) fn new(
        access: Arc<TaskAccess>,
        signature: IntentSignature,
        content: String,
    ) -> Self {
        Self {
            access,
            signature,
            content,
            steps: Arc::new(WorkQueue::new()),
            plan: StdMutex::new(None),
            plan_failures: StdMutex::new(Vec::new()),
            lifecycle: Lifecycle::new(),
        }
    }
}

impl RuntimeTrait for IntentRuntime {
    type Base = Intent;
    type Prepared = ();

    fn signature(&self) -> &IntentSignature {
        &self.signature
    }

    fn lifecycle(&self) -> &Lifecycle<IntentEvent, IntentResult> {
        &self.lifecycle
    }

    fn on_start(&self) -> ActorStartFuture<'_, Self::Prepared> {
        Box::pin(async move {
            Logger::log(format!("intent {} started", &self.signature,));
            if let Err(reason) = self.verdict() {
                self.fail(reason);
                return None;
            }
            Some(())
        })
    }

    fn dispatch(&self, event: IntentEvent) {
        match event {
            IntentEvent::SubintentUpdate(signature, status) => {
                self.on_subintent_update(signature, status);
            }
            IntentEvent::StepUpdate(signature, status) => {
                self.on_step_update(signature, status);
            }
            IntentEvent::RelayUpdate(signature, status) => {
                self.on_relay_update(signature, status);
            }
            IntentEvent::Cancel => self.cancel(),
        }
    }

    fn on_finish(&self, result: IntentResult) {
        self.send_task_update(ActorStatus::Complete(result));
    }
}

// -- Private -- //

impl IntentRuntime {
    pub(super) fn verdict(&self) -> Result<(), String> {
        let request = RelayRequest {
            signature: RelaySignature::new(self.signature.clone(), "intent-verdict".to_owned()),
            kind: RelayKind::IntentAnalyze,
        };
        let relay = Relay::new(Arc::clone(&self.access), request)?;
        if !self.access.insert(relay.clone()) {
            return Err(format!(
                "intent verdict relay {} already exists",
                relay.signature(),
            ));
        }
        relay.start();
        Ok(())
    }

    fn on_relay_update(&self, signature: RelaySignature, status: ActorStatus<RelayResult>) {
        if matches!(self.status(), ActorStatus::Complete(_)) {
            Logger::error(format!(
                "intent {} received relay {signature} update \
                 {status:?} after completion",
                &self.signature,
            ));
            return;
        }
        let ActorStatus::Complete(result) = status else {
            return;
        };
        if signature.name != "intent-verdict" {
            self.fail(format!(
                "intent received update from unexpected relay name `{}`",
                signature.name,
            ));
            return;
        }
        match result.kind {
            RelayResultKind::Succeed => {
                let draft = match StepDraft::parse(&result.output) {
                    Ok(draft) => draft,
                    Err(error) => {
                        self.fail(format!(
                            "intent relay {signature} returned malformed \
                             native tool calls: {error}",
                        ));
                        return;
                    }
                };
                if let Err(error) = self.dispatch_step_draft(draft) {
                    self.fail(format!(
                        "intent relay {signature} tool dispatch failed: \
                         {error}",
                    ));
                }
            }
            RelayResultKind::Failed => {
                self.finish(IntentResultKind::Failed, result.output);
            }
            RelayResultKind::Canceled => {
                self.finish(IntentResultKind::Canceled, result.output);
            }
        }
    }

    fn on_step_update(&self, signature: StepSignature, status: ActorStatus<StepResult>) {
        if matches!(self.status(), ActorStatus::Complete(_)) {
            Logger::error(format!(
                "intent {} received step {signature} update \
                 {status:?} after completion",
                &self.signature,
            ));
            return;
        }
        let ActorStatus::Complete(result) = status else {
            return;
        };
        let Some(updated) = self.steps.with_mut(&signature, |stored| {
            if stored.is_some() {
                return false;
            }
            *stored = Some(result.clone());
            true
        }) else {
            self.fail(format!("step {signature} not found"));
            return;
        };
        if !updated {
            Logger::error(format!(
                "intent {} received duplicate complete update from \
                 step {signature}",
                &self.signature,
            ));
            return;
        }
        match result.kind {
            StepResultKind::Succeed | StepResultKind::Failed => {
                if let Err(reason) = self.verdict() {
                    self.fail(reason);
                }
            }
            StepResultKind::Canceled => {
                self.finish(IntentResultKind::Canceled, "tool calls canceled".to_owned());
            }
        }
    }

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

    fn dispatch_step_draft(&self, draft: StepDraft) -> Result<(), String> {
        let workflow_call_count = draft
            .invocations
            .iter()
            .filter(|invocation| Self::is_workflow_tool(&invocation.name))
            .count();
        if workflow_call_count == 0 {
            let names = draft
                .invocations
                .iter()
                .map(|invocation| invocation.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            return self.create_step(draft).map_err(|error| {
                format!("execution tool calls [{names}] could not start: {error}")
            });
        }
        if draft.invocations.len() != 1 {
            let names = draft
                .invocations
                .iter()
                .map(|invocation| invocation.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(format!(
                "workflow tools require exactly one call and cannot be \
                 mixed with execution tools; received [{}]",
                names,
            ));
        }

        let invocation = draft
            .invocations
            .into_iter()
            .next()
            .ok_or_else(|| "workflow tool dispatch received no call".to_owned())?;
        match invocation.name.as_str() {
            WorkflowPlan::NAME => {
                let tool = WorkflowPlan::parse(&invocation.input).map_err(|error| {
                    format!(
                        "workflow tool `{}` arguments are invalid: {error}",
                        invocation.name,
                    )
                })?;
                self.create_plan(tool.draft)
                    .map_err(|error| format!("workflow tool `{}` failed: {error}", invocation.name))
            }
            WorkflowComplete::NAME => {
                let tool = WorkflowComplete::parse(&invocation.input).map_err(|error| {
                    format!(
                        "workflow tool `{}` arguments are invalid: {error}",
                        invocation.name,
                    )
                })?;
                self.finish(IntentResultKind::Succeed, tool.output);
                Ok(())
            }
            WorkflowInfeasible::NAME => {
                let tool = WorkflowInfeasible::parse(&invocation.input).map_err(|error| {
                    format!(
                        "workflow tool `{}` arguments are invalid: {error}",
                        invocation.name,
                    )
                })?;
                self.finish(IntentResultKind::Infeasible, tool.reason);
                Ok(())
            }
            _ => Err(format!(
                "workflow tool `{}` is not recognized",
                invocation.name,
            )),
        }
    }

    fn is_workflow_tool(name: &str) -> bool {
        matches!(
            name,
            WorkflowCallSummary::NAME
                | WorkflowPlan::NAME
                | WorkflowComplete::NAME
                | WorkflowInfeasible::NAME
        )
    }

    pub(super) fn cancel(&self) {
        if matches!(self.status(), ActorStatus::Complete(_)) {
            return;
        }
        self.cancel_plan();
        for (signature, result) in self.steps.entries() {
            if result.is_some() {
                continue;
            }
            let event = SessionEvent::Task(
                self.access.signature.clone(),
                TaskEvent::Step(signature.clone(), StepEvent::Cancel),
            );
            if self.access.session_tx.send(event).is_err() {
                Logger::warning(format!(
                    "intent {} step {signature} cancel failed: \
                     session stopped",
                    &self.signature,
                ));
            }
        }
        self.finish(IntentResultKind::Canceled, "intent canceled".to_owned());
    }

    pub(super) fn fail(&self, reason: String) {
        Logger::error(format!("intent {} failed: {reason}", &self.signature,));
        self.finish(IntentResultKind::Failed, reason);
    }

    pub(super) fn finish(&self, kind: IntentResultKind, output: String) {
        RuntimeTrait::finish(self, IntentResult { kind, output });
    }

    fn send_task_update(&self, status: ActorStatus<IntentResult>) {
        let event = match self.signature.parent.as_deref() {
            None => TaskEvent::Update(self.signature.clone(), status),
            Some(parent) => TaskEvent::Intent(
                parent.clone(),
                IntentEvent::SubintentUpdate(self.signature.clone(), status),
            ),
        };
        let task_event = SessionEvent::Task(self.access.signature.clone(), event);
        if self.access.session_tx.send(task_event).is_err() {
            Logger::warning(format!(
                "intent {} event send failed: session stopped",
                &self.signature,
            ));
        }
    }
}

#[allow(dead_code)]
fn assert_runtime_object_safe(runtime: &dyn RuntimeTrait<Base = Intent, Prepared = ()>) {
    let _ = runtime.run();
}
