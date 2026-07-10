use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::external::*;
use marix_common::{AsyncReceiver, AsyncSender, Logger, build_async_channel};
use marix_protocol::{
    Actor, Answer, InvocationEvent, InvocationSignature, PlanDraft, PlanEvent,
    PlanSignature, PlanStatus, RelayEvent, RelaySignature, RuntimeAsync,
    SessionEvent, StepEvent, StepSignature, TaskError, TaskEvent, TaskResult,
    TaskStatus,
};

use super::TaskState;
use crate::plan::{Plan, initial_plan};

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
        let Some(mut task_rx) = self
            .task_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
        else {
            Logger::warning(format!(
                "task {} runtime stopping: event receiver unavailable",
                &self.state.access.signature,
            ));
            return;
        };
        let Some(mut close_rx) = self
            .close_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
        else {
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
            TaskEvent::Update(signature, status) => {
                self.on_plan_update(signature, status);
                Ok(())
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
    fn create_plan(&self, draft: PlanDraft) {
        let access = &self.state.access;
        let signature = PlanSignature::new(
            access.signature.clone(),
            "plan".to_owned(),
        );
        let mut plan = match Plan::from_draft(access.clone(), signature, draft) {
            Ok(plan) => plan,
            Err(error) => {
                let reason = format!(
                    "failed to create task plan: {error:?}",
                );
                Logger::warning(format!(
                    "{reason} (task {})",
                    &access.signature,
                ));
                self.send_session_status(TaskStatus::Failed { reason });
                return;
            }
        };
        if let Err(error) = access.insert_plan(plan.clone()) {
            let reason = format!(
                "failed to insert task plan: {error:?}",
            );
            Logger::warning(format!(
                "{reason} (task {})",
                &access.signature,
            ));
            self.send_session_status(TaskStatus::Failed { reason });
            return;
        }
        plan.start();
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

    fn on_plan_update(&self, signature: PlanSignature, status: PlanStatus) {
        match status {
            PlanStatus::Success => {
                Logger::debug(format!(
                    "task {} plan {} completed",
                    &self.state.access.signature, &signature,
                ));
                self.on_plan_succeed(signature);
            }
            PlanStatus::Fail => {
                Logger::debug(format!(
                    "task {} runtime stopping: {:?}",
                    &self.state.access.signature,
                    TaskError::PlanFailed,
                ));
                self.send_session_status(TaskStatus::Failed {
                    reason: "plan failed".to_owned(),
                });
                self.close();
            }
        }
    }

    fn on_plan_succeed(&self, signature: PlanSignature) {
        let Some(model_step) = self.state.plans.with(&signature, |plan| {
            plan.state.model.clone()
        }) else {
            Logger::warning(format!(
                "task {} plan {} success ignored: plan not found",
                &self.state.access.signature, &signature,
            ));
            self.close();
            return;
        };

        let content = model_step.output();
        match serde_json::from_str::<PlanDraft>(&content) {
            Ok(plan_draft) => {
                self.create_plan(plan_draft);
                return;
            }
            Err(plan_error) => match serde_json::from_str::<Answer>(&content) {
                Ok(answer) => {
                    self.send_session_status(TaskStatus::Succeed(
                        TaskResult {
                            content: answer.answer,
                        },
                    ));
                }
                Err(answer_error) => {
                    let reason = format!(
                        "plan output did not parse as answer or plan draft: \
                         answer parse error: {answer_error}; \
                         plan draft parse error: {plan_error}",
                    );
                    Logger::warning(format!(
                        "task {} plan {} failed: {reason}",
                        &self.state.access.signature, &signature,
                    ));
                    self.send_session_status(TaskStatus::Failed {
                        reason,
                    });
                }
            },
        }
        self.close();
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
