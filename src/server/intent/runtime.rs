use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use marix_common::external::*;
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
use crate::relay::Relay;
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
    pub stage: StdMutex<StageType>,
    pub pending_stage:
        StdMutex<Option<(RelaySignature, StageType)>>,
    pub stage_sequence: AtomicUsize,
    pub tool_call_count: AtomicUsize,
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
            stage: StdMutex::new(StageType::IntentPlanning),
            pending_stage: StdMutex::new(None),
            stage_sequence: AtomicUsize::new(0),
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
    fn run_stage(&self, stage_type: StageType) -> Result<(), String> {
        let sequence =
            self.stage_sequence.fetch_add(1, Ordering::AcqRel) + 1;
        let signature = RelaySignature::new(
            self.signature.clone(),
            format!("stage-{sequence}"),
        );
        let assembler = StageAssembler::new(stage_type);
        let relay = Relay::new(
            Arc::clone(&self.access),
            signature.clone(),
            assembler,
            None,
        )?;
        {
            let mut pending = self
                .pending_stage
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if let Some((active, _)) = pending.as_ref() {
                return Err(format!(
                    "cannot start stage {stage_type:?}; relay {active} \
                     is still pending"
                ));
            }
            *pending = Some((signature.clone(), stage_type));
        }
        *self
            .stage
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = stage_type;
        if !self.access.insert(relay.clone()) {
            *self
                .pending_stage
                .lock()
                .unwrap_or_else(|error| error.into_inner()) = None;
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
        let stage_type = {
            let mut pending = self
                .pending_stage
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let Some((expected, stage_type)) = pending.as_ref()
            else {
                self.fail(format!(
                    "intent received relay {signature} update with no \
                     pending stage"
                ));
                return;
            };
            if expected != &signature {
                self.fail(format!(
                    "intent received update from unexpected relay \
                     {signature}; expected {expected}"
                ));
                return;
            }
            let stage_type = *stage_type;
            *pending = None;
            stage_type
        };
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
            StageResult::Plan(plan) => {
                if matches!(stage_type, StageType::IntentReplan) {
                    *self
                        .plan
                        .lock()
                        .unwrap_or_else(|error| error.into_inner()) =
                        None;
                }
                self.create_plan(PlanDraft {
                    intents: plan.subintents,
                })
            }
            StageResult::Reject(reject) => {
                self.info(format!(
                    "intent {} stage {stage_type:?} rejected: {}",
                    &self.signature, reject.reason,
                ));
                self.run_reject_transition(stage_type)
            }
            StageResult::Infeasible(infeasible) => {
                self.finish(
                    IntentResultKind::Infeasible,
                    infeasible.reason,
                );
                Ok(())
            }
            StageResult::IntentComplete(complete) => {
                self.finish(
                    IntentResultKind::Succeed,
                    complete.summary,
                );
                Ok(())
            }
            StageResult::NativeToolCalls(draft) => {
                let call_count = draft.invocations.len();
                self.create_step(draft)?;
                self.tool_call_count
                    .fetch_add(call_count, Ordering::AcqRel);
                Ok(())
            }
            StageResult::InvocationContinue(_)
            | StageResult::InvocationComplete(_) => Err(format!(
                "intent stage {stage_type:?} returned an invocation \
                 result"
            )),
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
            StageType::IntentToolCalling
            | StageType::InvocationContinue => {
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

    fn on_subintent_update(
        &self,
        signature: IntentSignature,
        status: ActorStatus<IntentResult>,
    ) {
        if matches!(self.status(), ActorStatus::Complete(_)) {
            self.error(format!(
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
            let plan = self
                .plan
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let Some(plan) = plan.as_ref() else {
                self.fail(format!(
                    "intent received subintent update from {signature} \
                     without an active plan"
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
                 {signature}"
            ));
            return;
        };
        match result.kind {
            IntentResultKind::Succeed => {
                if let Some(next) =
                    plan.subintents.get(index + 1).cloned()
                {
                    if let Err(reason) = self.start_subintent(next) {
                        self.fail(reason);
                    }
                } else if let Err(reason) = self.run_stage(
                    StageType::IntentSubintentComplete,
                ) {
                    self.fail(reason);
                }
            }
            IntentResultKind::Infeasible
            | IntentResultKind::Failed => {
                if let Err(reason) = self.record_plan_failure() {
                    self.fail(reason);
                    return;
                }
                *self
                    .plan
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = None;
                if let Err(reason) =
                    self.run_stage(StageType::IntentReplan)
                {
                    self.fail(reason);
                }
            }
            IntentResultKind::Canceled => {
                self.finish(IntentResultKind::Canceled, result.output);
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

    fn record_plan_failure(&self) -> Result<(), String> {
        let plan = self
            .plan
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
            .ok_or_else(|| {
                "cannot record a failure without an active plan"
                    .to_owned()
            })?;
        let goals = plan
            .subintents
            .iter()
            .map(|signature| {
                self.access.get_intent_content(signature).ok_or_else(
                    || {
                        format!(
                            "cannot snapshot plan: subintent \
                             {signature} was not found"
                        )
                    },
                )
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
                IntentResultKind::Failed
                | IntentResultKind::Infeasible => {
                    Some(result.output.clone())
                }
                IntentResultKind::Succeed
                | IntentResultKind::Canceled => None,
            })
            .ok_or_else(|| {
                "cannot record plan failure: no subintent failed or \
                 was infeasible"
                    .to_owned()
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

    fn start_subintent(
        &self,
        signature: IntentSignature,
    ) -> Result<(), String> {
        self.access
            .session_tx
            .send(SessionEvent::Task(
                self.access.signature.clone(),
                TaskEvent::IntentStart(signature.clone()),
            ))
            .map_err(|_| {
                format!(
                    "plan subintent {signature} start failed: \
                     session stopped"
                )
            })
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
