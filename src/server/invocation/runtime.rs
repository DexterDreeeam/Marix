use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::external::*;
use marix_common::{AsyncReceiver, AsyncSender, Logger, build_async_channel};
use marix_protocol::{
    ExecutionEvent, ExecutionRequest, ExecutionSignature, ExecutionStatus, ExecutorEvent,
    InvocationEvent, InvocationStatus, SessionEvent, StepEvent,
    TaskEvent,
};

use super::InvocationState;
use crate::task::TaskAccess;

pub struct InvocationRuntime {
    pub access: Arc<TaskAccess>,
    pub state: Arc<InvocationState>,
    pub invocation_rx: StdMutex<Option<AsyncReceiver<InvocationEvent>>>,
    pub close_tx: AsyncSender<()>,
    pub close_rx: StdMutex<Option<AsyncReceiver<()>>>,
}

impl InvocationRuntime {
    pub async fn run(&self) {
        let Some(mut invocation_rx) = self
            .invocation_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
        else {
            Logger::warning(format!(
                "invocation {} start ignored: already running",
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
            self.system_failure("invocation close receiver unavailable".to_owned());
            return;
        };
        self.create_execution();

        loop {
            self::tokio::select! {
                _ = close_rx.recv() => break,
                event = invocation_rx.recv() => {
                    let Some(event) = event else {
                        break;
                    };
                    self.dispatch(event);
                }
            }
        }
    }

    pub fn dispatch(&self, event: InvocationEvent) {
        match event {
            InvocationEvent::Update(execution, status) => {
                self.on_update(execution, status);
            }
            InvocationEvent::Processing {
                execution,
                seq,
                content,
            } => {
                self.on_processing(execution, seq, content);
            }
            InvocationEvent::Cancel => self.cancel(),
        }
    }
}

// -- Private -- //

impl InvocationRuntime {
    pub(crate) fn new(access: Arc<TaskAccess>, state: Arc<InvocationState>) -> Self {
        let (close_tx, close_rx) = build_async_channel();
        let invocation_rx = state
            .invocation_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
        Self {
            access,
            state,
            invocation_rx: StdMutex::new(invocation_rx),
            close_tx,
            close_rx: StdMutex::new(Some(close_rx)),
        }
    }

    fn create_execution(&self) {
        let signature = ExecutionSignature::new(
            self.state.signature.clone(),
            self.state.signature.name.clone(),
        );
        *self
            .state
            .execution_signature
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(signature.clone());
        if !self.send_executor_event(ExecutorEvent::ExecutionCreate(ExecutionRequest {
            signature,
            input: self.state.input.clone(),
        })) {
            self.system_failure("executor event send failed: session stopped".to_owned());
        }
    }

    fn on_processing(&self, execution: ExecutionSignature, seq: usize, content: String) {
        if self.status().is_terminal() {
            Logger::error(format!(
                "invocation {} received processing update from execution \
                 {execution} after completion",
                &self.state.signature,
            ));
            return;
        }
        let mut output = self
            .state
            .output
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(applied) = output.get(&seq) {
            if applied != &content {
                drop(output);
                self.system_failure(format!(
                    "execution {execution} sent conflicting output chunk \
                     {seq}"
                ));
            }
            return;
        }
        output.insert(seq, content);
    }

    fn on_update(&self, execution: ExecutionSignature, status: ExecutionStatus) {
        if self.status().is_terminal() {
            Logger::error(format!(
                "invocation {} received execution {execution} update \
                 {status:?} after completion",
                &self.state.signature,
            ));
            return;
        }
        let status = Self::map_execution_status(status);
        if !status.is_terminal() {
            return;
        }
        if let InvocationStatus::Succeed { seq_count } = status {
            let complete = {
                let output = self
                    .state
                    .output
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                output.len() == seq_count && (0..seq_count).all(|seq| output.contains_key(&seq))
            };
            if !complete {
                self.system_failure(format!(
                    "invocation {} completed with missing output chunks; \
                     expected {seq_count}",
                    &self.state.signature,
                ));
                return;
            }
            *self
                .state
                .final_signal
                .lock()
                .unwrap_or_else(|error| error.into_inner()) = Some(seq_count);
            self.set_status(InvocationStatus::Succeed { seq_count });
            self.send_step_update(InvocationStatus::Succeed { seq_count });
            self.close();
            return;
        }
        self.set_status(status.clone());
        self.send_step_update(status);
        self.close();
    }

    fn cancel(&self) {
        if self.status().is_terminal() {
            return;
        }
        let execution = self
            .state
            .execution_signature
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        if let Some(signature) = execution {
            if !self
                .send_executor_event(ExecutorEvent::Execution(signature, ExecutionEvent::Cancel))
            {
                self.system_failure("execution cancel send failed: session stopped".to_owned());
                return;
            }
        }
        self.set_status(InvocationStatus::Canceled);
        self.send_step_update(InvocationStatus::Canceled);
        self.close();
    }

    fn system_failure(&self, reason: String) {
        if self.status().is_terminal() {
            return;
        }
        Logger::error(format!(
            "invocation {} failed: {reason}",
            &self.state.signature,
        ));
        self.set_status(InvocationStatus::Failed);
        self.send_step_update(InvocationStatus::Failed);
        self.close();
    }

    fn status(&self) -> InvocationStatus {
        self.state
            .status
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    fn set_status(&self, status: InvocationStatus) {
        *self
            .state
            .status
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = status;
    }

    fn map_execution_status(status: ExecutionStatus) -> InvocationStatus {
        match status {
            ExecutionStatus::Created => InvocationStatus::Created,
            ExecutionStatus::Started => InvocationStatus::Started,
            ExecutionStatus::Canceled => InvocationStatus::Canceled,
            ExecutionStatus::Succeed { seq_count } => InvocationStatus::Succeed { seq_count },
            ExecutionStatus::Failed => InvocationStatus::Failed,
        }
    }

    fn send_step_update(&self, status: InvocationStatus) {
        let step = self.state.signature.step.clone();
        let event = SessionEvent::Task(
            step.intent.task.clone(),
            TaskEvent::Step(
                step,
                StepEvent::Update(
                    self.state.signature.clone(),
                    status,
                ),
            ),
        );
        if self.access.session_tx.send(event).is_err() {
            Logger::warning(format!(
                "invocation {} event send failed: session stopped",
                &self.state.signature,
            ));
        }
    }

    fn send_executor_event(&self, event: ExecutorEvent) -> bool {
        if self
            .access
            .session_tx
            .send(SessionEvent::Executor(event))
            .is_err()
        {
            return false;
        }
        true
    }

    fn close(&self) {
        if self.close_tx.send(()).is_err() {
            Logger::warning(format!(
                "invocation {} close signal failed",
                &self.state.signature,
            ));
        }
    }
}
