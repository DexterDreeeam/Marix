use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::Ordering;

use marix_common::external::*;
use marix_common::{AsyncReceiver, AsyncSender, Logger, build_async_channel};
use marix_protocol::{
    Actor, PlanError, PlanEvent, PlanStatus, RuntimeAsync, SessionEvent,
    StepEvent, StepStatus, TaskEvent,
};

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
                    plan_signature,
                    &signature,
                    &signature.task,
                ));
                Ok(())
            }
            PlanEvent::StepUpdate(status) => self.on_update(status),
            PlanEvent::Cancel => {
                self.cancel_steps();
                Err(PlanError::Canceled)
            }
        }
    }
}

// -- Private -- //

impl PlanRuntime {
    fn on_update(&self, status: StepStatus) -> Result<(), PlanError> {
        match status {
            StepStatus::Succeed => {
                if self.is_complete() {
                    self.send_task_event(TaskEvent::PlanUpdate(PlanStatus::Success));
                }
                Ok(())
            }
            StepStatus::Canceled | StepStatus::Failed => {
                self.send_task_event(TaskEvent::PlanUpdate(PlanStatus::Fail));
                Ok(())
            }
            StepStatus::Created | StepStatus::Started => Ok(()),
        }
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

    fn is_complete(&self) -> bool {
        let total_steps = self.state.call.len() + 1;
        let completed_steps = self
            .state
            .completed_steps
            .fetch_add(1, Ordering::Relaxed)
            + 1;
        completed_steps >= total_steps
    }

    fn send_task_event(&self, event: TaskEvent) {
        if self
            .state
            .access
            .session_tx
            .send(SessionEvent::Task(
                self.state.signature.task.clone(),
                event,
            ))
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
