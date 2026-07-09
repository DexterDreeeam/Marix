use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::external::*;
use marix_common::{Logger, build_async_channel};
use marix_protocol::{
    Actor, InvocationEvent, InvocationRequest, InvocationSignature, InvocationStatus,
    InvocationStepKind, ModelStepKind, PlanDraft, PlanEvent, PlanSignature, PlanStatus, RelayEvent,
    RelayRequest, RelaySignature, RelayStatus, RuntimeAsync, SessionEvent, StepEvent, StepKind,
    StepSignature, TaskError, TaskEvent, TaskStatus,
};

use super::TaskState;
use crate::plan::Plan;
use crate::prompt::{AnalysisPrompt, InitialPrompt, Prompt};
use crate::step::Step;

pub struct TaskRuntime {
    state: Arc<TaskState>,
    task_rx: StdMutex<Option<tokio::mpsc::UnboundedReceiver<TaskEvent>>>,
    close_tx: tokio::mpsc::UnboundedSender<()>,
    close_rx: StdMutex<Option<tokio::mpsc::UnboundedReceiver<()>>>,
}

impl TaskRuntime {
    pub fn new(state: Arc<TaskState>) -> Self {
        let (close_tx, close_rx) = build_async_channel();
        let task_rx = state
            .task_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
        Self {
            state,
            task_rx: StdMutex::new(task_rx),
            close_tx,
            close_rx: StdMutex::new(Some(close_rx)),
        }
    }
}

impl RuntimeAsync<TaskEvent, TaskError> for TaskRuntime {
    async fn run(&self) {
        self.send_session_status(TaskStatus::Started);
        let access = &self.state.access;
        Logger::log(format!("task {} started", &access.signature));
        Step::trigger_initial_plan(access.clone());
        let Some(mut task_rx) = self.take_task_rx() else {
            Logger::warning(format!(
                "task {} runtime stopping: event receiver unavailable",
                &self.state.access.signature,
            ));
            return;
        };
        let Some(mut close_rx) = self.take_close_rx() else {
            Logger::warning(format!(
                "task {} runtime stopping: close receiver unavailable",
                &self.state.access.signature,
            ));
            return;
        };
        loop {
            self::tokio::select! {
                _ = close_rx.recv() => break,
                event = task_rx.recv() => {
                    let Some(event) = event else {
                        break;
                    };
                    if let Err(error) = self.dispatch(event) {
                        Logger::debug(format!(
                            "task {} runtime stopping: {error:?}",
                            &self.state.access.signature,
                        ));
                        break;
                    }
                }
            }
        }
    }

    fn close(&self) {
        if let Err(error) = self.close_tx.send(()) {
            Logger::warning(format!(
                "task {} close signal failed: {error}",
                &self.state.access.signature,
            ));
        }
    }

    fn dispatch(&self, event: TaskEvent) -> Result<(), TaskError> {
        match event {
            TaskEvent::Plan(signature, event) => {
                self.dispatch_plan(signature, event);
                Ok(())
            }
            TaskEvent::PlanCreate(draft) => {
                self.create_plan(draft);
                Ok(())
            }
            TaskEvent::PlanUpdate(status) => {
                let failed = matches!(status, PlanStatus::Fail);
                self.on_plan_update(status);
                if failed {
                    Err(TaskError::PlanFailed)
                } else {
                    Ok(())
                }
            }
            TaskEvent::Cancel => {
                self.cancel_task();
                Err(TaskError::Canceled)
            }
        }
    }
}

// -- Private -- //

impl TaskRuntime {
    fn take_task_rx(&self) -> Option<tokio::mpsc::UnboundedReceiver<TaskEvent>> {
        self.task_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
    }

    fn take_close_rx(&self) -> Option<tokio::mpsc::UnboundedReceiver<()>> {
        self.close_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
    }

    fn create_plan(&self, draft: PlanDraft) {
        let access = &self.state.access;
        let signature = PlanSignature::new(access.signature.clone(), "plan".to_owned());
        let plan = match Plan::from_draft(access.clone(), signature.clone(), draft) {
            Ok(plan) => plan,
            Err(error) => {
                let reason = format!("discarding invalid plan draft: {error:?}");
                Logger::warning(format!("{reason} (task {})", &access.signature));
                self.send_session_status(TaskStatus::Failed { reason });
                return;
            }
        };
        Logger::debug(format!(
            "running plan {} with {} step(s) (task {})",
            &signature,
            plan.run_steps().len(),
            &access.signature,
        ));
        let step_signatures = plan.run_step_signatures();
        if let Err(error) =
            self.state
                .plan_hub
                .insert(signature.clone(), plan.clone(), step_signatures)
        {
            let reason = format!("failed to insert task plan: {error:?}");
            Logger::error(format!("{reason} (task {})", &access.signature));
            self.send_session_status(TaskStatus::Failed { reason });
            return;
        }
        self.state.plan_hub.with_mut(&signature, |plan| {
            plan.run();
        });
        if plan.run_steps().is_empty() {
            self.on_plan_update(PlanStatus::Success);
            return;
        }
        for step in plan.run_steps() {
            self.start_step(step);
        }
    }

    fn dispatch_plan(&self, signature: PlanSignature, event: PlanEvent) {
        match event {
            PlanEvent::Step(step_signature, step_event) => {
                self.dispatch_step(signature, step_signature, step_event);
                return;
            }
            event => self.dispatch_plan_to_worker(signature, event),
        }
    }

    fn dispatch_step(&self, plan: PlanSignature, signature: StepSignature, event: StepEvent) {
        match event {
            StepEvent::InvocationCreate(request) => {
                self.create_invocation(&signature, request);
            }
            StepEvent::Invocation(invocation, event) => {
                self.dispatch_invocation(invocation, event);
            }
            StepEvent::InvocationUpdate(status) => {
                self.on_invocation_update(plan, signature, status);
            }
            StepEvent::RelayCreate(request) => {
                self.create_relay(&signature, request);
            }
            StepEvent::Relay(relay, event) => {
                self.dispatch_relay(relay, event);
            }
            StepEvent::RelayUpdate(status) => {
                self.on_relay_update(plan, signature, status);
            }
            StepEvent::Cancel => {
                self.cancel_step(plan, signature);
            }
        }
    }

    fn start_step(&self, step: &Step) {
        if self
            .state
            .steps
            .with(&step.signature().id, |_| ())
            .is_some()
        {
            Logger::warning(format!(
                "step {} start ignored: step already exists (task {})",
                step.signature(),
                &step.signature().task,
            ));
            return;
        }
        if self.state.steps.size() >= 10 {
            let reason = "task step limit exceeded".to_owned();
            Logger::warning(format!(
                "step {} rejected: {reason} (task {})",
                step.signature(),
                &step.signature().task,
            ));
            step.fail_with_reason(reason);
            return;
        }
        self.state
            .steps
            .insert_or_update(step.signature().id.clone(), step.clone());
        self.state.steps.with_mut(&step.signature().id, Step::run);
        match step.kind() {
            StepKind::Model(_) => self.create_model_relay(step),
            StepKind::Invocation(InvocationStepKind::Invocation(request)) => {
                self.create_invocation(step.signature(), request.clone());
            }
            StepKind::Invocation(InvocationStepKind::Cancel) => {
                self.fail_step(step, "invocation cancel step has no target".to_owned());
            }
            StepKind::Invocation(InvocationStepKind::Kill) => {
                self.fail_step(
                    step,
                    "invocation kill is not supported by the current protocol".to_owned(),
                );
            }
            StepKind::Intent | StepKind::User(_) => {
                self.fail_step(step, "task step kind is not supported yet".to_owned());
            }
        }
    }

    fn create_model_relay(&self, step: &Step) {
        Logger::debug(format!(
            "model step {} started (task {})",
            step.signature(),
            &step.signature().task,
        ));
        let signature = RelaySignature::new(
            self.state.access.signature.clone(),
            step.signature().plan.clone(),
            step.signature().clone(),
            step.signature().name.clone(),
        );
        let request = RelayRequest {
            signature,
            prompt: self.model_prompt(step),
        };
        self.create_relay(step.signature(), request);
    }

    fn create_invocation(&self, step: &StepSignature, request: InvocationRequest) {
        if &request.signature.step != step {
            Logger::warning(format!(
                "step {} rejected invocation {}: signature step mismatch",
                step, &request.signature,
            ));
            return;
        }
        let signature = request.signature.clone();
        if self
            .state
            .invocation_hub
            .create(self.state.access.clone(), request)
        {
            self.dispatch_invocation(signature, InvocationEvent::ExecutionCreate);
        }
    }

    fn dispatch_invocation(&self, signature: InvocationSignature, event: InvocationEvent) {
        let event_name = format!("{event:?}");
        match self
            .state
            .invocation_hub
            .with(&signature, |invocation| invocation.dispatch(event))
        {
            Some(()) => {}
            None => {
                Logger::warning(format!(
                    "invocation {} event {event_name} not dispatched: invocation not found",
                    &signature,
                ));
            }
        }
    }

    fn create_relay(&self, step: &StepSignature, request: RelayRequest) {
        if &request.signature.step != step {
            Logger::warning(format!(
                "step {} rejected relay {}: signature step mismatch",
                step, &request.signature,
            ));
            return;
        }
        self.state
            .relay_hub
            .create(self.state.access.clone(), request);
    }

    fn dispatch_relay(&self, signature: RelaySignature, event: RelayEvent) {
        let event_name = format!("{event:?}");
        match self
            .state
            .relay_hub
            .with(&signature, |relay| relay.dispatch(event))
        {
            Some(()) => {}
            None => {
                Logger::warning(format!(
                    "relay {} event {event_name} not dispatched: relay not found",
                    &signature,
                ));
            }
        }
    }

    fn on_invocation_update(
        &self,
        plan: PlanSignature,
        signature: StepSignature,
        status: InvocationStatus,
    ) {
        match status {
            InvocationStatus::Succeed { .. } => {
                let content = self.invocation_content(&signature);
                self.complete_step(plan, signature, content);
            }
            InvocationStatus::Canceled => {
                self.fail_step_by_signature(plan, signature, "invocation canceled".to_owned());
            }
            InvocationStatus::Failed => {
                self.fail_step_by_signature(plan, signature, "invocation failed".to_owned());
            }
            status => self.forward_step_event(plan, signature, StepEvent::InvocationUpdate(status)),
        }
    }

    fn on_relay_update(&self, plan: PlanSignature, signature: StepSignature, status: RelayStatus) {
        match status {
            RelayStatus::Succeed { .. } => {
                let content = String::new();
                self.complete_step(plan, signature, content);
            }
            RelayStatus::Canceled => {
                self.fail_step_by_signature(plan, signature, "relay canceled".to_owned());
            }
            RelayStatus::Failed => {
                self.fail_step_by_signature(plan, signature, "relay failed".to_owned());
            }
            status => self.forward_step_event(plan, signature, StepEvent::RelayUpdate(status)),
        }
    }

    fn cancel_step(&self, plan: PlanSignature, signature: StepSignature) {
        let Some(step) = self.step(&plan, &signature) else {
            return;
        };
        match step.kind() {
            StepKind::Invocation(InvocationStepKind::Invocation(request)) => {
                self.dispatch_invocation(request.signature.clone(), InvocationEvent::Cancel);
            }
            StepKind::Model(_) => {
                self.fail_step(&step, "model cancellation is not supported".to_owned());
            }
            StepKind::Intent | StepKind::User(_) | StepKind::Invocation(_) => {
                self.fail_step(&step, "step canceled".to_owned());
            }
        }
    }

    fn complete_step(&self, plan: PlanSignature, signature: StepSignature, content: String) {
        if !self.complete_step_queue(&signature) {
            return;
        }
        if self.state.plan_hub.complete_step(&signature).is_none() {
            return;
        }
        if let Some(step) = self.step(&plan, &signature) {
            step.complete(content);
        }
    }

    fn fail_step_by_signature(
        &self,
        plan: PlanSignature,
        signature: StepSignature,
        reason: String,
    ) {
        if !self.complete_step_queue(&signature) {
            return;
        }
        if let Some(step) = self.step(&plan, &signature) {
            step.fail_with_reason(reason);
        }
    }

    fn fail_step(&self, step: &Step, reason: String) {
        let _ = self.complete_step_queue(step.signature());
        step.fail_with_reason(reason);
    }

    fn complete_step_queue(&self, signature: &StepSignature) -> bool {
        let is_working = self
            .state
            .steps
            .working_list()
            .iter()
            .any(|step| step.signature().id == signature.id);
        if !is_working {
            Logger::warning(format!(
                "step {} completion ignored: step is not working (task {})",
                signature, &signature.task,
            ));
            return false;
        }
        self.state.steps.complete(signature.id.clone());
        true
    }

    fn forward_step_event(&self, plan: PlanSignature, signature: StepSignature, event: StepEvent) {
        self.dispatch_plan_to_worker(plan, PlanEvent::Step(signature, event));
    }

    fn dispatch_plan_to_worker(&self, signature: PlanSignature, event: PlanEvent) {
        let event_name = format!("{event:?}");
        match self.state.plan_hub.get(&signature) {
            Ok(plan) => {
                if plan.sender().send(event).is_err() {
                    Logger::warning(format!(
                        "plan {} event {event_name} dispatch failed: worker stopped (task {})",
                        &signature, &signature.task,
                    ));
                }
            }
            Err(_) => {
                Logger::error(format!(
                    "plan {} event {event_name} not dispatched: plan not found (task {})",
                    &signature, &signature.task,
                ));
            }
        }
    }

    fn step(&self, plan: &PlanSignature, signature: &StepSignature) -> Option<Step> {
        match self.state.plan_hub.get(plan) {
            Ok(plan) => plan.step(signature),
            Err(_) => {
                Logger::error(format!(
                    "step {} not found: plan {} not found (task {})",
                    signature, plan, &signature.task,
                ));
                None
            }
        }
    }

    fn invocation_content(&self, _signature: &StepSignature) -> String {
        String::new()
    }

    fn model_prompt(&self, step: &Step) -> String {
        match step.kind() {
            StepKind::Model(ModelStepKind::Initial) => {
                let session_context = self
                    .state
                    .access
                    .session_context
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .snapshot();
                InitialPrompt::new(self.state.access.user_request.clone(), session_context).prompt()
            }
            StepKind::Model(ModelStepKind::Analysis) => {
                let session_context = self
                    .state
                    .access
                    .session_context
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .snapshot();
                let plan_stringify = self.state.plan_hub.stringify();
                AnalysisPrompt::new(
                    self.state.access.user_request.clone(),
                    step.description().to_owned(),
                    plan_stringify.current_plan_text(),
                    plan_stringify.pending_intentions_text(),
                    session_context,
                )
                .prompt()
            }
            _ => self.state.access.user_request.clone(),
        }
    }

    fn on_plan_update(&self, status: PlanStatus) {
        match status {
            PlanStatus::Success => {
                Logger::debug(format!(
                    "task {} plan completed",
                    &self.state.access.signature
                ));
            }
            PlanStatus::Fail => {
                self.send_session_status(TaskStatus::Failed {
                    reason: "plan failed".to_owned(),
                });
            }
        }
    }

    fn cancel_task(&self) {
        Logger::log(format!("task {} canceled", &self.state.access.signature));
        for signature in self.state.plan_hub.list().unwrap_or_default() {
            self.dispatch_plan(signature, PlanEvent::Cancel);
        }
        self.send_session_status(TaskStatus::Canceled);
    }

    fn send_session_status(&self, status: TaskStatus) {
        if self
            .state
            .access
            .session_tx
            .send(SessionEvent::TaskUpdate(status))
            .is_err()
        {
            Logger::warning(format!(
                "task {} status update failed: session worker stopped",
                &self.state.access.signature,
            ));
        }
    }
}
