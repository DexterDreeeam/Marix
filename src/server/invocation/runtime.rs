use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::external::*;
use marix_common::{AsyncReceiver, AsyncSender, Logger, build_async_channel};
use marix_protocol::{
    ExecutionEvent, ExecutionRequest, ExecutionSignature, ExecutionStatus, ExecutorEvent,
    InvocationError, InvocationEvent, InvocationStatus, PlanEvent, RuntimeAsync, SessionEvent,
    StepEvent, TaskEvent,
};

use super::state::InvocationState;

pub(super) struct InvocationRuntime {
    state: Arc<InvocationState>,
    invocation_rx: StdMutex<Option<AsyncReceiver<InvocationEvent>>>,
    close_tx: AsyncSender<()>,
    close_rx: StdMutex<Option<AsyncReceiver<()>>>,
}

impl InvocationRuntime {
    pub(super) fn new(state: Arc<InvocationState>) -> Self {
        let (close_tx, close_rx) = build_async_channel();
        let invocation_rx = state
            .invocation_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
        Self {
            state,
            invocation_rx: StdMutex::new(invocation_rx),
            close_tx,
            close_rx: StdMutex::new(Some(close_rx)),
        }
    }
}

impl RuntimeAsync<InvocationEvent, InvocationError> for InvocationRuntime {
    async fn run(&self) {
        let Some(mut invocation_rx) = self
            .invocation_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
        else {
            Logger::warning(format!(
                "invocation {} runtime stopping: event receiver unavailable",
                &self.state.signature,
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
                "invocation {} runtime stopping: close receiver unavailable",
                &self.state.signature,
            ));
            return;
        };
        Logger::debug(format!(
            "invocation {} runtime loop starting",
            &self.state.signature,
        ));
        loop {
            self::tokio::select! {
                _ = close_rx.recv() => break,
                event = invocation_rx.recv() => {
                    let Some(event) = event else {
                        break;
                    };
                    if let Err(error) = self.dispatch(event) {
                        Logger::debug(format!(
                            "invocation {} runtime stopping: {error:?}",
                            &self.state.signature,
                        ));
                        break;
                    }
                }
            }
        }
        Logger::debug(format!(
            "invocation {} runtime loop stopped",
            &self.state.signature,
        ));
    }

    fn close(&self) {
        if let Err(error) = self.close_tx.send(()) {
            Logger::warning(format!(
                "invocation {} close signal failed: {error}",
                &self.state.signature,
            ));
        }
    }

    fn dispatch(&self, event: InvocationEvent) -> Result<(), InvocationError> {
        match event {
            InvocationEvent::ExecutionCreate => {
                self.create_execution();
                Ok(())
            }
            InvocationEvent::ExecutionUpdate(status) => self.on_update(status),
            InvocationEvent::Cancel => {
                self.cancel_execution();
                Err(InvocationError::Canceled)
            }
        }
    }
}

// -- Private -- //

impl InvocationRuntime {
    fn create_execution(&self) {
        let execution_signature = ExecutionSignature::new(
            self.state.signature.clone(),
            self.state.signature.name.clone(),
        );
        let request = ExecutionRequest {
            signature: execution_signature.clone(),
            input: self.state.input.clone(),
        };
        *self
            .state
            .execution_signature
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(execution_signature);
        self.send_executor_event(ExecutorEvent::ExecutionCreate(request));
    }

    fn cancel_execution(&self) {
        let execution_signature = {
            self.state
                .execution_signature
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .clone()
        };
        if let Some(signature) = execution_signature {
            self.send_executor_event(ExecutorEvent::Execution(signature, ExecutionEvent::Cancel));
        } else {
            Logger::warning(format!(
                "invocation {} cancel requested before execution create",
                &self.state.signature,
            ));
        }
    }

    fn on_update(&self, status: ExecutionStatus) -> Result<(), InvocationError> {
        let mut succeed_seq_count = None;
        match status {
            ExecutionStatus::Created | ExecutionStatus::Started => {}
            ExecutionStatus::Processing { seq, content } => {
                self.state
                    .output
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .insert(seq, content.clone());
                self.send_step_event(InvocationStatus::Processing { seq, content });
            }
            ExecutionStatus::Canceled => return Err(InvocationError::ExecutionCanceled),
            ExecutionStatus::Succeed { seq_count } => {
                succeed_seq_count = Some(seq_count);
                *self
                    .state
                    .final_signal
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = Some(seq_count);
            }
            ExecutionStatus::Failed => return Err(InvocationError::ExecutionFailed),
        }

        if !Self::is_complete(&self.state) {
            return Ok(());
        }

        let Some(seq_count) = succeed_seq_count.or_else(|| {
            *self
                .state
                .final_signal
                .lock()
                .unwrap_or_else(|error| error.into_inner())
        }) else {
            return Ok(());
        };
        self.send_step_event(InvocationStatus::Succeed { seq_count });
        Ok(())
    }

    fn send_step_event(&self, status: InvocationStatus) {
        let event = SessionEvent::Task(
            self.state.signature.task.clone(),
            TaskEvent::Plan(
                self.state.signature.plan.clone(),
                PlanEvent::Step(
                    self.state.signature.step.clone(),
                    StepEvent::InvocationUpdate(status),
                ),
            ),
        );
        if self.state.access.session_tx.send(event).is_err() {
            Logger::warning(format!(
                "invocation {} step event failed: session worker stopped",
                &self.state.signature,
            ));
        }
    }

    fn send_executor_event(&self, event: ExecutorEvent) {
        if self
            .state
            .access
            .session_tx
            .send(SessionEvent::Executor(event))
            .is_err()
        {
            Logger::warning(format!(
                "invocation {} executor event failed: session worker stopped",
                &self.state.signature,
            ));
        }
    }

    fn is_complete(state: &InvocationState) -> bool {
        let final_signal = *state
            .final_signal
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let Some(count) = final_signal else {
            return false;
        };
        state
            .output
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .len()
            == count
    }
}
