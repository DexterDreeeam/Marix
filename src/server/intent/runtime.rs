use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::{
    Actor, ActorStartFuture, ActorStatus, Lifecycle, Logger, Runtime as RuntimeTrait, WorkQueue,
};
use marix_protocol::{
    IntentEvent, IntentResult, IntentResultKind, IntentSignature, IntentVerdict, PlanEvent,
    PlanResultKind, PlanSignature, RelayRequest, RelayResultKind, RelaySignature, SessionEvent,
    StepEvent, StepResult, StepResultKind, StepSignature, TaskEvent,
};

use super::Intent;
use crate::prompt::Prompt;
use crate::relay::Relay;
use crate::task::TaskAccess;

pub struct IntentRuntime {
    pub access: Arc<TaskAccess>,
    pub signature: IntentSignature,
    pub content: String,
    pub steps: Arc<WorkQueue<StepSignature, Option<StepResult>>>,
    pub plan: StdMutex<Option<PlanSignature>>,
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
            IntentEvent::PlanUpdate(signature, status) => {
                self.on_plan_update(signature, status);
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

    fn on_finish(&self) {
        self.send_task_update(ActorStatus::Complete);
    }
}

// -- Private -- //

impl IntentRuntime {
    fn verdict(&self) -> Result<(), String> {
        let prompt =
            std::panic::catch_unwind(|| Prompt::load("IntentAnalyze")).map_err(|payload| {
                let detail = if let Some(message) = payload.downcast_ref::<String>() {
                    message.clone()
                } else if let Some(message) = payload.downcast_ref::<&str>() {
                    (*message).to_owned()
                } else {
                    "unknown prompt loading panic".to_owned()
                };
                format!("failed to load IntentAnalyze prompt: {detail}",)
            })?;
        let prompt = prompt
            .prompt()
            .map_err(|error| format!("failed to render IntentAnalyze prompt: {error}"))?;
        let request = RelayRequest {
            signature: RelaySignature::new(
                self.signature.clone(),
                None,
                "intent-verdict".to_owned(),
            ),
            prompt,
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

    fn on_plan_update(&self, signature: PlanSignature, status: ActorStatus) {
        if self.status() == ActorStatus::Complete {
            Logger::error(format!(
                "intent {} received plan {signature} update \
                 {status:?} after completion",
                &self.signature,
            ));
            return;
        }
        if status != ActorStatus::Complete {
            return;
        }
        let plan = self
            .plan
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        if plan.as_ref() != Some(&signature) {
            self.fail(format!(
                "intent received update from unexpected plan \
                 {signature}",
            ));
            return;
        }
        let Some(result) = self.access.get_result(&signature) else {
            self.fail(format!("plan {signature} completed without a result",));
            return;
        };
        match result.kind {
            PlanResultKind::Succeed => {
                self.finish(IntentResultKind::Succeed, result.output);
            }
            PlanResultKind::Infeasible => {
                self.finish(IntentResultKind::Infeasible, result.output);
            }
            PlanResultKind::Canceled => {
                self.finish(IntentResultKind::Canceled, result.output);
            }
            PlanResultKind::Failed => self.fail(result.output),
        }
    }

    fn on_relay_update(&self, signature: RelaySignature, status: ActorStatus) {
        if self.status() == ActorStatus::Complete {
            Logger::error(format!(
                "intent {} received relay {signature} update \
                 {status:?} after completion",
                &self.signature,
            ));
            return;
        }
        if status != ActorStatus::Complete {
            return;
        }
        let Some(result) = self.access.get_result(&signature) else {
            self.fail(format!("relay {signature} completed without a result",));
            return;
        };
        let plan = self
            .plan
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        if let Some(plan) = plan {
            self.send_plan_event(
                plan,
                PlanEvent::RelayUpdate(signature, ActorStatus::Complete),
            );
            return;
        }
        match result.kind {
            RelayResultKind::Succeed => {
                let verdict = match IntentVerdict::parse(&result.output) {
                    Ok(verdict) => verdict,
                    Err(error) => {
                        self.fail(format!(
                            "intent verdict from relay \
                                 {signature} is malformed: {error}",
                        ));
                        return;
                    }
                };
                self.on_verdict(verdict);
            }
            RelayResultKind::Failed => {
                self.finish(IntentResultKind::Failed, result.output);
            }
            RelayResultKind::Canceled => {
                self.finish(IntentResultKind::Canceled, result.output);
            }
        }
    }

    fn on_step_update(&self, signature: StepSignature, status: ActorStatus) {
        if self.status() == ActorStatus::Complete {
            Logger::error(format!(
                "intent {} received step {signature} update \
                 {status:?} after completion",
                &self.signature,
            ));
            return;
        }
        if status != ActorStatus::Complete {
            return;
        }
        let Some(result) = self.access.get_result(&signature) else {
            self.fail(format!("step {signature} completed without a result",));
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
                self.finish(IntentResultKind::Canceled, result.output);
            }
        }
    }

    fn on_verdict(&self, verdict: IntentVerdict) {
        match verdict {
            IntentVerdict::Step(draft) => {
                if let Err(reason) = self.create_step(draft) {
                    self.fail(reason);
                }
            }
            IntentVerdict::Plan(draft) => {
                if let Err(reason) = self.create_plan(draft) {
                    self.fail(reason);
                }
            }
            IntentVerdict::Complete { output } => {
                self.finish(IntentResultKind::Succeed, output);
            }
            IntentVerdict::Infeasible { reason } => {
                self.finish(IntentResultKind::Infeasible, reason);
            }
        }
    }

    pub(super) fn cancel(&self) {
        if self.status() == ActorStatus::Complete {
            return;
        }
        let plan = self
            .plan
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        if let Some(plan) = plan {
            self.send_plan_event(plan, PlanEvent::Cancel);
        }
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

    fn send_task_update(&self, status: ActorStatus) {
        let event = match self.signature.parent.clone() {
            None => TaskEvent::Update(self.signature.clone(), status),
            Some(parent) => {
                TaskEvent::Plan(parent, PlanEvent::Update(self.signature.clone(), status))
            }
        };
        let task_event = SessionEvent::Task(self.access.signature.clone(), event);
        if self.access.session_tx.send(task_event).is_err() {
            Logger::warning(format!(
                "intent {} event send failed: session stopped",
                &self.signature,
            ));
        }
    }

    fn send_plan_event(&self, signature: PlanSignature, event: PlanEvent) {
        let task_event = SessionEvent::Task(
            self.access.signature.clone(),
            TaskEvent::Plan(signature, event),
        );
        if self.access.session_tx.send(task_event).is_err() {
            Logger::warning(format!(
                "intent {} plan event send failed: session stopped",
                &self.signature,
            ));
        }
    }
}

#[allow(dead_code)]
fn assert_runtime_object_safe(runtime: &dyn RuntimeTrait<Base = Intent, Prepared = ()>) {
    let _ = runtime.run();
}
