use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::Ordering;

use marix_common::external::*;
use marix_common::{AsyncReceiver, AsyncSender, Logger, build_async_channel};
use marix_protocol::{
    Actor, PlanError, PlanEvent, PlanStatus, RuntimeAsync, SessionEvent, StepEvent, StepKind,
    StepSignature, StepStatus, TaskEvent,
};

use super::helper::model_input;
use super::state::PlanState;

pub(super) struct PlanRuntime {
    state: Arc<PlanState>,
    plan_rx: StdMutex<Option<AsyncReceiver<PlanEvent>>>,
    close_tx: AsyncSender<()>,
    close_rx: StdMutex<Option<AsyncReceiver<()>>>,
}

impl PlanRuntime {
    pub(super) fn new(state: Arc<PlanState>) -> Self {
        let (close_tx, close_rx) = build_async_channel();
        let plan_rx = state
            .plan_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
        Self {
            state,
            plan_rx: StdMutex::new(plan_rx),
            close_tx,
            close_rx: StdMutex::new(Some(close_rx)),
        }
    }
}

impl RuntimeAsync<PlanEvent, PlanError> for PlanRuntime {
    async fn run(&self) {
        let Some(mut plan_rx) = self
            .plan_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
        else {
            let signature = &self.state.signature;
            Logger::warning(format!(
                "plan {} runtime stopping: event receiver unavailable (task {})",
                signature, &signature.task,
            ));
            return;
        };
        let Some(mut close_rx) = self
            .close_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
        else {
            let signature = &self.state.signature;
            Logger::warning(format!(
                "plan {} runtime stopping: close receiver unavailable (task {})",
                signature, &signature.task,
            ));
            return;
        };
        self.start_steps();
        let signature = &self.state.signature;
        Logger::debug(format!(
            "plan {} runtime loop starting (task {})",
            signature, &signature.task,
        ));
        loop {
            self::tokio::select! {
                _ = close_rx.recv() => break,
                event = plan_rx.recv() => {
                    let Some(event) = event else {
                        break;
                    };
                    if let Err(error) = self.dispatch(event) {
                        let signature = &self.state.signature;
                        Logger::debug(format!(
                            "plan {} runtime stopping: {error:?} (task {})",
                            signature, &signature.task,
                        ));
                        break;
                    }
                }
            }
        }
        let signature = &self.state.signature;
        Logger::debug(format!(
            "plan {} runtime loop stopped (task {})",
            signature, &signature.task,
        ));
    }

    fn close(&self) {
        if let Err(error) = self.close_tx.send(()) {
            let signature = &self.state.signature;
            Logger::warning(format!(
                "plan {} close signal failed: {error} (task {})",
                signature, &signature.task,
            ));
        }
    }

    fn dispatch(&self, event: PlanEvent) -> Result<(), PlanError> {
        match event {
            PlanEvent::Step(signature, event) => {
                let plan_signature = &self.state.signature;
                Logger::error(format!(
                    "plan {} received unsupported step event {event:?} for {} (task {})",
                    plan_signature, &signature, &signature.task,
                ));
                Ok(())
            }
            PlanEvent::Update(signature, status) => self.on_update(signature, status),
            PlanEvent::Cancel => {
                self.cancel_steps();
                self.close();
                Err(PlanError::Canceled)
            }
        }
    }
}

// -- Private -- //

impl PlanRuntime {
    fn start_steps(&self) {
        if self.state.call.is_empty() {
            self.start_model();
            return;
        }

        for step in &self.state.call {
            let mut step = step.clone();
            if !self.state.access.insert_step(step.clone()) {
                self.fail_plan(format!(
                    "call step {} could not be inserted",
                    step.signature(),
                ));
                return;
            }
            step.start();
        }
    }

    fn on_update(&self, signature: StepSignature, status: StepStatus) -> Result<(), PlanError> {
        let step = self
            .state
            .call
            .iter()
            .find(|step| step.signature() == &signature)
            .or_else(|| (self.state.model.signature() == &signature).then_some(&self.state.model));
        let Some(step) = step else {
            Logger::warning(format!(
                "plan {} ignored update from unknown step {} (task {})",
                &self.state.signature, &signature, &self.state.signature.task,
            ));
            return Ok(());
        };

        match status {
            StepStatus::Canceled | StepStatus::Failed => {
                self.fail_plan(format!(
                    "step {} ended with status {status:?}",
                    step.signature(),
                ));
                Ok(())
            }
            StepStatus::Created | StepStatus::Started => Ok(()),
            StepStatus::Succeed => match step.kind() {
                StepKind::Invocation(_) => {
                    if self.calls_complete() {
                        self.start_model();
                    }
                    Ok(())
                }
                StepKind::Model(_) => {
                    self.send_task_event(TaskEvent::Update(
                        self.state.signature.clone(),
                        PlanStatus::Success,
                    ));
                    self.close();
                    Ok(())
                }
                kind => {
                    self.fail_plan(format!(
                        "step {} succeeded with unsupported kind {kind:?}",
                        step.signature(),
                    ));
                    Ok(())
                }
            },
        }
    }

    fn calls_complete(&self) -> bool {
        self.state
            .call
            .iter()
            .all(|step| step.status() == StepStatus::Succeed)
    }

    fn cancel_steps(&self) {
        for step in &self.state.call {
            self.send_task_event(TaskEvent::Plan(
                self.state.signature.clone(),
                PlanEvent::Step(step.signature().clone(), StepEvent::Cancel),
            ));
        }
        let step = &self.state.model;
        self.send_task_event(TaskEvent::Plan(
            self.state.signature.clone(),
            PlanEvent::Step(step.signature().clone(), StepEvent::Cancel),
        ));
    }

    fn start_model(&self) {
        if self.state.model_once.swap(true, Ordering::AcqRel) {
            Logger::debug(format!(
                "plan {} ignored duplicate model start (task {})",
                &self.state.signature, &self.state.signature.task,
            ));
            return;
        }

        let mut step = self.state.model.clone();
        let input = model_input(&self.state.background, &self.state.call);
        step.set_input(input);
        if !self.state.access.insert_step(step.clone()) {
            self.fail_plan(format!(
                "model step {} could not be inserted",
                step.signature(),
            ));
            return;
        }
        step.start();
    }

    fn fail_plan(&self, reason: String) {
        Logger::error(format!(
            "plan {} failed: {reason} (task {})",
            &self.state.signature, &self.state.signature.task,
        ));
        self.send_task_event(TaskEvent::Update(
            self.state.signature.clone(),
            PlanStatus::Fail,
        ));
        self.close();
    }

    fn send_task_event(&self, event: TaskEvent) {
        if self
            .state
            .access
            .session_tx
            .send(SessionEvent::Task(self.state.signature.task.clone(), event))
            .is_err()
        {
            let signature = &self.state.signature;
            Logger::warning(format!(
                "plan {} update failed: session worker stopped (task {})",
                signature, &signature.task,
            ));
        }
    }
}
