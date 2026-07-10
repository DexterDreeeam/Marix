use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::external::*;
use marix_common::{AsyncReceiver, AsyncSender, Logger, build_async_channel};
use marix_protocol::{
    Actor, InvocationEvent, InvocationSignature, InvocationStepKind, ModelStepKind, PlanDraft,
    PlanEvent, PlanSignature, PlanStatus, RelayEvent, RelayRequest, RelaySignature, RuntimeAsync,
    SessionEvent, StepEvent, StepKind, StepSignature, StepStatus, TaskError, TaskEvent,
    TaskStatus,
};

use super::TaskState;
use crate::plan::{Plan, PlanStringify, initial_plan};
use crate::prompt::{AnalysisPrompt, InitialPrompt, Prompt};
use crate::step::Step;

pub struct TaskRuntime {
    state: Arc<TaskState>,
    task_rx: StdMutex<Option<AsyncReceiver<TaskEvent>>>,
    close_tx: AsyncSender<()>,
    close_rx: StdMutex<Option<AsyncReceiver<()>>>,
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
        self.create_plan(initial_plan(access.user_request.clone()));
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
        Logger::debug(format!(
            "task {} runtime loop starting",
            &self.state.access.signature,
        ));
        loop {
            self::tokio::select! {
                _ = close_rx.recv() => break,
                event = task_rx.recv() => {
                    let Some(event) = event else {
                        break;
                    };
                    match event {
                        TaskEvent::Plan(signature, event) => {
                            self.dispatch_plan(signature, event);
                        }
                        TaskEvent::PlanCreate(draft) => {
                            self.create_plan(draft);
                        }
                        TaskEvent::PlanUpdate(status) => {
                            let failed = matches!(status, PlanStatus::Fail);
                            self.on_plan_update(status);
                            if failed {
                                Logger::debug(format!(
                                    "task {} runtime stopping: {:?}",
                                    &self.state.access.signature,
                                    TaskError::PlanFailed,
                                ));
                                break;
                            }
                        }
                        TaskEvent::Cancel => {
                            self.cancel_task();
                            Logger::debug(format!(
                                "task {} runtime stopping: {:?}",
                                &self.state.access.signature,
                                TaskError::Canceled,
                            ));
                            break;
                        }
                    }
                }
            }
        }
        Logger::debug(format!(
            "task {} runtime loop stopped",
            &self.state.access.signature,
        ));
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
    fn take_task_rx(&self) -> Option<AsyncReceiver<TaskEvent>> {
        self.task_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
    }

    fn take_close_rx(&self) -> Option<AsyncReceiver<()>> {
        self.close_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
    }

    fn create_plan(&self, draft: PlanDraft) {
        let access = &self.state.access;
        let signature = PlanSignature::new(
            access.signature.clone(),
            "plan".to_owned(),
        );
        let plan = match Plan::from_draft(access.clone(), signature, draft) {
            Ok(plan) => plan,
            Err(error) => {
                self.fail_plan_create(format!(
                    "failed to create task plan: {error:?}",
                ));
                return;
            }
        };
        let call = plan.state.call.clone();
        let model = plan.state.model.clone();
        let start_model = call.is_empty();
        if let Err(error) = access.insert_plan(plan.clone()) {
            self.fail_plan_create(format!(
                "failed to insert task plan: {error:?}",
            ));
            return;
        }
        let mut worker = plan;
        worker.start();
        for step in &call {
            self.start_plan_step(step);
        }
        if start_model {
            self.start_plan_step(&model);
        }
    }

    fn fail_plan_create(&self, reason: String) {
        let access = &self.state.access;
        Logger::warning(format!("{reason} (task {})", &access.signature));
        self.send_session_status(TaskStatus::Failed { reason });
    }

    fn start_plan_step(&self, step: &Step) {
        if self.state.steps.with(step.signature(), |_| ()).is_some() {
            Logger::warning(format!(
                "step {} start ignored: step already exists (task {})",
                step.signature(),
                &step.signature().task,
            ));
            return;
        }
        if self.state.steps.size() >= 10 {
            Logger::warning(format!(
                "step {} rejected: task step limit exceeded (task {})",
                step.signature(),
                &step.signature().task,
            ));
            self.dispatch_plan(
                step.signature().plan.clone(),
                PlanEvent::StepUpdate(StepStatus::Failed),
            );
            return;
        }
        self.state
            .steps
            .insert_or_update(step.signature().clone(), step.clone());
        let mut worker = step.clone();
        worker.start();
        self.dispatch_plan(
            step.signature().plan.clone(),
            PlanEvent::StepUpdate(StepStatus::Created),
        );
        self.dispatch_step_create_event(step);
    }

    fn dispatch_step_create_event(&self, step: &Step) {
        match step.kind() {
            StepKind::Model(_) => {
                let signature = RelaySignature::new(
                    self.state.access.signature.clone(),
                    step.signature().plan.clone(),
                    step.signature().clone(),
                    step.signature().name.clone(),
                );
                step.dispatch(StepEvent::RelayCreate(RelayRequest {
                    signature,
                    prompt: self.build_model_prompt(step),
                }));
            }
            StepKind::Invocation(InvocationStepKind::Invocation(request)) => {
                step.dispatch(StepEvent::InvocationCreate(request.clone()));
            }
            StepKind::Invocation(InvocationStepKind::Cancel) => {
                Logger::warning(format!(
                    "step {} failed: invocation cancel step has no target",
                    step.signature(),
                ));
                self.dispatch_plan(
                    step.signature().plan.clone(),
                    PlanEvent::StepUpdate(StepStatus::Failed),
                );
            }
            StepKind::Invocation(InvocationStepKind::Kill) => {
                Logger::warning(format!(
                    "step {} failed: invocation kill is not supported",
                    step.signature(),
                ));
                self.dispatch_plan(
                    step.signature().plan.clone(),
                    PlanEvent::StepUpdate(StepStatus::Failed),
                );
            }
            StepKind::Intent | StepKind::User(_) => {
                Logger::warning(format!(
                    "step {} failed: task step kind is not supported yet",
                    step.signature(),
                ));
                self.dispatch_plan(
                    step.signature().plan.clone(),
                    PlanEvent::StepUpdate(StepStatus::Failed),
                );
            }
        }
    }

    fn build_model_prompt(&self, step: &Step) -> String {
        let access = &self.state.access;
        match step.kind() {
            StepKind::Model(ModelStepKind::Initial) => {
                let session_context = access
                    .session_context
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .snapshot();
                InitialPrompt::new(access.user_request.clone(), session_context)
                    .prompt()
            }
            StepKind::Model(ModelStepKind::Analysis) => {
                let session_context = access
                    .session_context
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .snapshot();
                let plan_stringify = self.stringify_plans();
                AnalysisPrompt::new(
                    access.user_request.clone(),
                    step.description().to_owned(),
                    plan_stringify.current_plan_text(),
                    plan_stringify.pending_intentions_text(),
                    session_context,
                )
                .prompt()
            }
            _ => access.user_request.clone(),
        }
    }

    fn stringify_plans(&self) -> PlanStringify {
        let mut plans = self.state.plans.working_list();
        plans.extend(self.state.plans.complete_list());
        PlanStringify::new(plans)
    }

    fn dispatch_plan(&self, signature: PlanSignature, event: PlanEvent) {
        match event {
            PlanEvent::Step(step_signature, step_event) => {
                self.dispatch_step(signature, step_signature, step_event);
            }
            event => {
                let mut event = Some(event);
                match self
                    .state
                    .plans
                    .with(&signature, |plan| {
                        plan.dispatch(event.take().unwrap_or_else(|| {
                            unreachable!("plan event already dispatched")
                        }))
                    })
                {
                    Some(()) => {}
                    None => {
                        let event = event.unwrap_or_else(|| {
                            unreachable!("plan event dispatched without a plan")
                        });
                        Logger::error(format!(
                            "plan {} event {event:?} not dispatched: plan not found (task {})",
                            &signature, &signature.task,
                        ));
                    }
                }
            }
        }
    }

    fn dispatch_step(&self, plan: PlanSignature, signature: StepSignature, event: StepEvent) {
        match event {
            StepEvent::Invocation(invocation, event) => {
                self.dispatch_invocation(invocation, event);
            }
            StepEvent::Relay(relay, event) => {
                self.dispatch_relay(relay, event);
            }
            event => {
                let mut event = Some(event);
                match self
                    .state
                    .steps
                    .with(&signature, |step| {
                        step.dispatch(event.take().unwrap_or_else(|| {
                            unreachable!("step event already dispatched")
                        }))
                    })
                {
                    Some(()) => {}
                    None => {
                        let event = event.unwrap_or_else(|| {
                            unreachable!("step event dispatched without a step")
                        });
                        Logger::error(format!(
                            "step {} event {event:?} not dispatched: step not found in plan {}",
                            &signature, &plan,
                        ));
                    }
                }
            }
        }
    }

    fn dispatch_invocation(&self, signature: InvocationSignature, event: InvocationEvent) {
        let mut event = Some(event);
        match self
            .state
            .invocations
            .with(&signature, |invocation| {
                invocation.dispatch(event.take().unwrap_or_else(|| {
                    unreachable!("invocation event already dispatched")
                }))
            })
        {
            Some(()) => {}
            None => {
                let event = event.unwrap_or_else(|| {
                    unreachable!("invocation event dispatched without an invocation")
                });
                Logger::warning(format!(
                    "invocation {} event {event:?} not dispatched: invocation not found",
                    &signature,
                ));
            }
        }
    }

    fn dispatch_relay(&self, signature: RelaySignature, event: RelayEvent) {
        let mut event = Some(event);
        match self
            .state
            .relays
            .with(&signature, |relay| {
                relay.dispatch(event.take().unwrap_or_else(|| {
                    unreachable!("relay event already dispatched")
                }))
            })
        {
            Some(()) => {}
            None => {
                let event = event.unwrap_or_else(|| {
                    unreachable!("relay event dispatched without a relay")
                });
                Logger::warning(format!(
                    "relay {} event {event:?} not dispatched: relay not found",
                    &signature,
                ));
            }
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
        for plan in self.state.plans.working_list() {
            self.dispatch_plan(plan.state.signature.clone(), PlanEvent::Cancel);
        }
        for plan in self.state.plans.complete_list() {
            self.dispatch_plan(plan.state.signature.clone(), PlanEvent::Cancel);
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
