use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use marix_common::{
    Actor, ActorStartFuture, ActorStatus, Lifecycle,
    Runtime as RuntimeTrait, WorkQueue,
};
use marix_protocol::{
    IntentEvent, IntentResult, IntentResultKind, IntentSignature,
    PlanDraft, PlanResult, RelayResult, RelayResultKind,
    RelaySignature, SessionEvent, StepDraft, StepEvent, StepResult,
    StepResultKind, StepSignature, TaskEvent, TaskLogger, TaskLogging,
};

use super::Intent;
use crate::plan::Plan;
use crate::relay::{Relay, RelayOwner};
use crate::stage::{StageAssembler, StageResult, StageType};
use crate::step::Step;
use crate::task::TaskAccess;

pub struct IntentRuntime {
    pub access: Arc<TaskAccess>,
    pub signature: IntentSignature,
    pub content: String,
    pub steps: Arc<WorkQueue<StepSignature, Option<StepResult>>>,
    pub plan: StdMutex<Option<Plan>>,
    pub plan_failures: StdMutex<Vec<PlanResult>>,
    pub tool_call_count: AtomicUsize,
    pub lifecycle: Lifecycle<IntentEvent, IntentResult>,
    stage: StdMutex<StageType>,
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
            stage: StdMutex::new(StageType::IntentPlanning),
            tool_call_count: AtomicUsize::new(0),
            lifecycle: Lifecycle::new(),
        }
    }
}

impl TaskLogging for IntentRuntime {
    fn logger(&self) -> TaskLogger {
        self.access.logger()
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
            self.info(format!("intent {} started", &self.signature));
            if let Err(reason) =
                self.run_stage(StageType::IntentPlanning)
            {
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
    pub(super) fn run_stage(
        &self,
        stage_type: StageType,
    ) -> Result<(), String> {
        let signature = RelaySignature::new(
            self.signature.clone(),
            format!("stage-{stage_type:?}"),
        );
        let request = StageAssembler::for_intent(stage_type)
            .assemble(&self.access, signature.clone())?;
        let relay = Relay::new(
            Arc::clone(&self.access),
            request,
            RelayOwner::Intent,
        )?;
        *self
            .stage
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = stage_type;
        if !self.access.insert(relay.clone()) {
            return Err(format!(
                "intent stage relay {signature} already exists"
            ));
        }
        relay.start();
        Ok(())
    }

    fn on_relay_update(
        &self,
        signature: RelaySignature,
        status: ActorStatus<RelayResult>,
    ) {
        if matches!(self.status(), ActorStatus::Complete(_)) {
            self.error(format!(
                "intent {} received relay {signature} update \
                 {status:?} after completion",
                &self.signature,
            ));
            return;
        }
        let ActorStatus::Complete(result) = status else {
            return;
        };
        let stage_type = *self
            .stage
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        match result.kind {
            RelayResultKind::Succeed => {
                let stage_result =
                    match stage_type.parse_result(&result.output) {
                        Ok(result) => result,
                        Err(reason) => {
                            self.fail(format!(
                                "intent relay {signature} returned an \
                                 invalid stage result: {reason}"
                            ));
                            return;
                        }
                    };
                if let Err(reason) =
                    self.apply_stage_result(stage_type, stage_result)
                {
                    self.fail(reason);
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

    fn apply_stage_result(
        &self,
        stage_type: StageType,
        result: StageResult,
    ) -> Result<(), String> {
        match result {
            StageResult::Plan { subintents, .. } => {
                if matches!(stage_type, StageType::IntentReplan) {
                    *self
                        .plan
                        .lock()
                        .unwrap_or_else(|error| error.into_inner()) =
                        None;
                }
                self.create_plan(PlanDraft {
                    intents: subintents,
                })
            }
            StageResult::Reject { reason } => {
                self.info(format!(
                    "intent {} stage {stage_type:?} rejected: {}",
                    &self.signature, reason,
                ));
                self.run_reject_transition(stage_type)
            }
            StageResult::Infeasible { reason } => {
                self.finish(IntentResultKind::Infeasible, reason);
                Ok(())
            }
            StageResult::IntentComplete { summary, .. } => {
                self.finish(IntentResultKind::Succeed, summary);
                Ok(())
            }
            StageResult::NativeToolCalls(draft) => {
                let call_count = draft.invocations.len();
                self.create_step(draft)?;
                self.tool_call_count
                    .fetch_add(call_count, Ordering::AcqRel);
                Ok(())
            }
        }
    }

    fn run_reject_transition(
        &self,
        stage_type: StageType,
    ) -> Result<(), String> {
        let next = match stage_type {
            StageType::IntentPlanning => {
                StageType::IntentToolCalling
            }
            StageType::IntentReplan => {
                *self
                    .plan
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = None;
                StageType::IntentInfeasible
            }
            StageType::IntentInfeasible => {
                self.tool_call_count.store(0, Ordering::Release);
                StageType::IntentToolCalling
            }
            StageType::IntentSubintentComplete => {
                StageType::IntentReplan
            }
            StageType::IntentComplete => {
                if self.tool_call_count.load(Ordering::Acquire) < 4 {
                    StageType::IntentToolCalling
                } else {
                    StageType::IntentInfeasible
                }
            }
            StageType::IntentToolCalling => {
                return Err(format!(
                    "stage {stage_type:?} cannot return Reject"
                ));
            }
        };
        self.run_stage(next)
    }

    fn on_step_update(
        &self,
        signature: StepSignature,
        status: ActorStatus<StepResult>,
    ) {
        if matches!(self.status(), ActorStatus::Complete(_)) {
            self.error(format!(
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
            self.error(format!(
                "intent {} received duplicate complete update from \
                 step {signature}",
                &self.signature,
            ));
            return;
        }
        match result.kind {
            StepResultKind::Succeed | StepResultKind::Failed => {
                if let Err(reason) =
                    self.run_stage(StageType::IntentComplete)
                {
                    self.fail(reason);
                }
            }
            StepResultKind::Canceled => {
                self.finish(
                    IntentResultKind::Canceled,
                    "tool calls canceled".to_owned(),
                );
            }
        }
    }

    pub(super) fn create_step(
        &self,
        draft: StepDraft,
    ) -> Result<(), String> {
        if self
            .plan
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .is_some()
        {
            return Err(
                "intent cannot create a direct step after creating a plan"
                    .to_owned(),
            );
        }
        if draft.invocations.is_empty() {
            return Err(
                "intent tool-calling stage returned no invocation"
                    .to_owned(),
            );
        }
        let signature = StepSignature::new(
            self.signature.clone(),
            format!("step-{}", self.steps.size() + 1),
        );
        let step = Step::from_draft(
            Arc::clone(&self.access),
            signature.clone(),
            draft,
        )?;
        if !self.access.insert(step.clone()) {
            return Err(format!("step {signature} is duplicated"));
        }
        self.steps.insert(signature, None);
        step.start();
        Ok(())
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
                self.warning(format!(
                    "intent {} step {signature} cancel failed: \
                     session stopped",
                    &self.signature,
                ));
            }
        }
        self.finish(
            IntentResultKind::Canceled,
            "intent canceled".to_owned(),
        );
    }

    pub(super) fn fail(&self, reason: String) {
        self.error(format!(
            "intent {} failed: {reason}",
            &self.signature,
        ));
        self.finish(IntentResultKind::Failed, reason);
    }

    pub(super) fn finish(
        &self,
        kind: IntentResultKind,
        output: String,
    ) {
        RuntimeTrait::finish(self, IntentResult { kind, output });
    }

    fn send_task_update(&self, status: ActorStatus<IntentResult>) {
        let event = match self.signature.parent.as_deref() {
            None => TaskEvent::Update(self.signature.clone(), status),
            Some(parent) => TaskEvent::Intent(
                parent.clone(),
                IntentEvent::SubintentUpdate(
                    self.signature.clone(),
                    status,
                ),
            ),
        };
        let task_event =
            SessionEvent::Task(self.access.signature.clone(), event);
        if self.access.session_tx.send(task_event).is_err() {
            self.warning(format!(
                "intent {} event send failed: session stopped",
                &self.signature,
            ));
        }
    }
}

#[allow(dead_code)]
fn assert_runtime_object_safe(
    runtime: &dyn RuntimeTrait<Base = Intent, Prepared = ()>,
) {
    let _ = runtime.run();
}
