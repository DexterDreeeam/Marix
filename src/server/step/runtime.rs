use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::external::*;
use marix_common::{AsyncReceiver, AsyncSender, Logger, build_async_channel};
use marix_protocol::{
    Actor, InvocationEvent, InvocationRequest, InvocationStatus, InvocationStepKind,
    PlanEvent, RelayRequest, RelayStatus, RuntimeAsync, SessionEvent, StepError,
    StepEvent, StepKind, StepSignature, StepStatus, TaskEvent,
};

use super::state::StepState;
use crate::invocation::Invocation;
use crate::relay::Relay;

pub(super) struct StepRuntime {
    pub(super) state: Arc<StepState>,
    step_rx: StdMutex<Option<AsyncReceiver<StepEvent>>>,
    close_tx: AsyncSender<()>,
    close_rx: StdMutex<Option<AsyncReceiver<()>>>,
}

impl StepRuntime {
    pub(super) fn new(state: Arc<StepState>) -> Self {
        let (close_tx, close_rx) = build_async_channel();
        let step_rx = state
            .step_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
        Self {
            state,
            step_rx: StdMutex::new(step_rx),
            close_tx,
            close_rx: StdMutex::new(Some(close_rx)),
        }
    }

}

impl RuntimeAsync<StepEvent, StepError> for StepRuntime {
    async fn run(&self) {
        Self::send_step_update(&self.state, StepStatus::Started);
        let Some(mut step_rx) = self
            .step_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
        else {
            Logger::warning(format!(
                "step {} runtime stopping: event receiver unavailable (task {})",
                &self.state.signature, &self.state.signature.task,
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
                "step {} runtime stopping: close receiver unavailable (task {})",
                &self.state.signature, &self.state.signature.task,
            ));
            return;
        };
        Logger::debug(format!(
            "step {} runtime loop starting (task {})",
            &self.state.signature, &self.state.signature.task,
        ));
        loop {
            self::tokio::select! {
                _ = close_rx.recv() => break,
                event = step_rx.recv() => {
                    let Some(event) = event else {
                        break;
                    };
                    if let Err(error) = self.dispatch(event) {
                        Logger::debug(format!(
                            "step {} runtime stopping: {error:?} (task {})",
                            &self.state.signature, &self.state.signature.task,
                        ));
                        break;
                    }
                }
            }
        }
        Logger::debug(format!(
            "step {} runtime loop stopped (task {})",
            &self.state.signature, &self.state.signature.task,
        ));
    }

    fn close(&self) {
        if let Err(error) = self.close_tx.send(()) {
            Logger::warning(format!(
                "step {} close signal failed: {error} (task {})",
                &self.state.signature, &self.state.signature.task,
            ));
        }
    }

    fn dispatch(&self, event: StepEvent) -> Result<(), StepError> {
        match event {
            StepEvent::Invocation(signature, event) => {
                Logger::error(format!(
                    "step {} received invalid invocation event for {}",
                    &self.state.signature, &signature,
                ));
                let _ = event;
                Ok(())
            }
            StepEvent::InvocationCreate(request) => {
                self.create_invocation(request);
                Ok(())
            }
            StepEvent::InvocationUpdate(status) => {
                self.on_invocation_update(status);
                Ok(())
            }
            StepEvent::Relay(signature, event) => {
                Logger::error(format!(
                    "step {} received invalid relay event for {}",
                    &self.state.signature, &signature,
                ));
                let _ = event;
                Ok(())
            }
            StepEvent::RelayCreate(request) => {
                self.create_relay(request);
                Ok(())
            }
            StepEvent::RelayUpdate(status) => {
                self.on_relay_update(status);
                Ok(())
            }
            StepEvent::Cancel => {
                self.on_cancel();
                Err(StepError::Canceled)
            }
        }
    }
}

// -- Private -- //

impl StepRuntime {
    pub(super) fn signature(&self) -> &StepSignature {
        &self.state.signature
    }

    fn complete(&self, _content: String) {
        Self::send_step_update(&self.state, StepStatus::Succeed);
    }

    fn fail(&self, reason: String) {
        Logger::error(format!(
            "step {} failed: {reason} (task {})",
            &self.state.signature, &self.state.signature.task,
        ));
        Self::send_step_update(&self.state, StepStatus::Failed);
    }

    fn on_invocation_update(&self, status: InvocationStatus) {
        match status {
            InvocationStatus::Created => {
                Logger::debug(format!(
                    "step {} invocation created (task {})",
                    self.signature(),
                    &self.signature().task,
                ));
            }
            InvocationStatus::Started => {
                Logger::debug(format!(
                    "step {} invocation started (task {})",
                    self.signature(),
                    &self.signature().task,
                ));
            }
            InvocationStatus::Processing { .. } => {
                Logger::debug(format!(
                    "step {} invocation update (task {})",
                    self.signature(),
                    &self.signature().task,
                ));
            }
            InvocationStatus::Canceled => {
                self.fail("invocation canceled".to_owned());
            }
            InvocationStatus::Succeed { .. } => {
                self.complete(String::new());
            }
            InvocationStatus::Failed => {
                self.fail("invocation failed".to_owned());
            }
        }
    }

    fn on_relay_update(&self, status: RelayStatus) {
        match status {
            RelayStatus::Created => {
                Logger::debug(format!(
                    "step {} relay created (task {})",
                    self.signature(),
                    &self.signature().task,
                ));
            }
            RelayStatus::Started => {
                Logger::debug(format!(
                    "step {} relay started (task {})",
                    self.signature(),
                    &self.signature().task,
                ));
            }
            RelayStatus::Processing { .. } => {
                Logger::debug(format!(
                    "step {} relay update (task {})",
                    self.signature(),
                    &self.signature().task,
                ));
            }
            RelayStatus::Canceled => {
                self.fail("relay canceled".to_owned());
            }
            RelayStatus::Succeed { .. } => {
                self.complete(String::new());
            }
            RelayStatus::Failed => {
                self.fail("relay failed".to_owned());
            }
        }
    }

    fn create_invocation(&self, request: InvocationRequest) {
        if &request.signature.step != &self.state.signature {
            Logger::warning(format!(
                "step {} rejected invocation {}: signature step mismatch",
                &self.state.signature, &request.signature,
            ));
            return;
        }
        let invocation = Invocation::new(self.state.access.clone(), request);
        if self.state.access.insert_invocation(invocation.clone()) {
            let mut worker = invocation;
            worker.start();
            worker.dispatch(InvocationEvent::ExecutionCreate);
        }
    }

    fn create_relay(&self, request: RelayRequest) {
        if &request.signature.step != &self.state.signature {
            Logger::warning(format!(
                "step {} rejected relay {}: signature step mismatch",
                &self.state.signature, &request.signature,
            ));
            return;
        }
        let relay = Relay::new(self.state.access.clone(), request);
        if self.state.access.insert_relay(relay.clone()) {
            let mut worker = relay;
            worker.start();
        }
    }

    fn on_cancel(&self) {
        match &self.state.kind {
            StepKind::Invocation(InvocationStepKind::Invocation(request)) => {
                Logger::warning(format!(
                    "step {} canceled without forwarding invocation cancel to {}",
                    &self.state.signature, &request.signature,
                ));
                Self::send_step_update(&self.state, StepStatus::Canceled);
            }
            StepKind::Model(_) => {
                Logger::warning(format!(
                    "model step {} cancel requested, but model cancellation is not supported",
                    &self.state.signature,
                ));
                self.fail("model cancellation is not supported".to_owned());
            }
            StepKind::Intent | StepKind::User(_) | StepKind::Invocation(_) => {
                Logger::warning(format!("step {} canceled", &self.state.signature));
                Self::send_step_update(&self.state, StepStatus::Canceled);
            }
        }
    }

    fn send_step_update(state: &StepState, status: StepStatus) {
        let event = SessionEvent::Task(
            state.signature.task.clone(),
            TaskEvent::Plan(
                state.signature.plan.clone(),
                PlanEvent::StepUpdate(status),
            ),
        );
        if state.access.session_tx.send(event).is_err() {
            Logger::warning(format!(
                "step {} update failed: session worker stopped (task {})",
                &state.signature, &state.signature.task,
            ));
        }
    }
}
