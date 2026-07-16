use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::external::*;
use marix_common::{AsyncReceiver, AsyncSender, Logger, build_async_channel};
use marix_protocol::{
    ExecutionEvent, ExecutionRequest, ExecutionSignature, ExecutionStatus, ExecutorEvent,
    InvocationEvent, InvocationResult, InvocationResultKind, InvocationStatus, SessionEvent,
    StepEvent, TaskEvent,
};

use super::InvocationState;

pub struct InvocationRuntime {
    pub state: Arc<InvocationState>,
    pub close_tx: AsyncSender<()>,
    pub close_rx: StdMutex<Option<AsyncReceiver<()>>>,
}

impl InvocationRuntime {
    pub async fn run(&self) {
        let Some(mut invocation_rx) = self
            .state
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
        self.set_status(InvocationStatus::Running);
        let Some(mut close_rx) = self
            .close_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
        else {
            self.finish(
                InvocationResultKind::Failed,
                "invocation close receiver unavailable".to_owned(),
            );
            return;
        };
        if let Err(reason) = self.create_execution() {
            self.finish(InvocationResultKind::Failed, reason);
            return;
        }

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
    pub(crate) fn new(state: Arc<InvocationState>) -> Self {
        let (close_tx, close_rx) = build_async_channel();
        Self {
            state,
            close_tx,
            close_rx: StdMutex::new(Some(close_rx)),
        }
    }

    fn create_execution(&self) -> Result<(), String> {
        let signature = ExecutionSignature::new(
            self.state.signature.clone(),
            self.state.signature.name.clone(),
        );
        *self
            .state
            .execution
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(signature.clone());
        self.send_executor_event(ExecutorEvent::ExecutionCreate(ExecutionRequest {
            signature,
            input: self.state.input.clone(),
        }))
    }

    fn on_processing(&self, execution: ExecutionSignature, seq: usize, content: String) {
        if self.status().is_terminal() {
            Logger::error(format!(
                "invocation {} received processing update from execution {execution} after completion",
                &self.state.signature,
            ));
            return;
        }
        self.state
            .output
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(seq, content);
    }

    fn on_update(&self, execution: ExecutionSignature, status: ExecutionStatus) {
        if self.status().is_terminal() {
            Logger::error(format!(
                "invocation {} received execution {execution} update {status:?} after completion",
                &self.state.signature,
            ));
            return;
        }
        match status {
            ExecutionStatus::Created | ExecutionStatus::Started => {}
            ExecutionStatus::Succeed { seq_count } => {
                let Some(output) = self.complete_output(seq_count) else {
                    self.finish(
                        InvocationResultKind::Failed,
                        format!(
                            "invocation {} completed with missing output chunks; expected {seq_count}",
                            &self.state.signature,
                        ),
                    );
                    return;
                };
                *self
                    .state
                    .final_signal
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = Some(seq_count);
                self.finish(InvocationResultKind::Succeed, output);
            }
            ExecutionStatus::Canceled => {
                self.finish(
                    InvocationResultKind::Canceled,
                    format!("execution {execution} canceled"),
                );
            }
            ExecutionStatus::Failed => {
                self.finish(
                    InvocationResultKind::Failed,
                    format!("execution {execution} failed"),
                );
            }
        }
    }

    fn complete_output(&self, seq_count: usize) -> Option<String> {
        let output = self
            .state
            .output
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if output.len() != seq_count || (0..seq_count).any(|seq| !output.contains_key(&seq)) {
            return None;
        }
        Some(
            (0..seq_count)
                .filter_map(|seq| output.get(&seq))
                .cloned()
                .collect(),
        )
    }

    fn cancel(&self) {
        if self.status().is_terminal() {
            return;
        }
        let execution = self
            .state
            .execution
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        if let Some(signature) = execution {
            if let Err(reason) = self.send_executor_event(ExecutorEvent::Execution(
                signature,
                ExecutionEvent::Cancel,
            )) {
                Logger::warning(format!(
                    "invocation {} execution cancel failed: {reason}",
                    &self.state.signature,
                ));
            }
        }
        self.finish(
            InvocationResultKind::Canceled,
            "invocation canceled".to_owned(),
        );
    }

    fn finish(&self, kind: InvocationResultKind, output: String) {
        let result = InvocationResult { kind, output };
        self.set_status(InvocationStatus::Complete(result));
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
        let mut current = self
            .state
            .status
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if current.is_terminal() {
            return;
        }
        let send_update = matches!(&status, InvocationStatus::Complete(_));
        *current = status.clone();
        drop(current);
        if send_update {
            self.send_step_update(status);
        }
    }

    fn send_step_update(&self, status: InvocationStatus) {
        let step = self.state.signature.step.clone();
        let event = SessionEvent::Task(
            step.intent.task.clone(),
            TaskEvent::Step(
                step,
                StepEvent::Update(self.state.signature.clone(), status),
            ),
        );
        if self.state.access.session_tx.send(event).is_err() {
            Logger::warning(format!(
                "invocation {} event send failed: session stopped",
                &self.state.signature,
            ));
        }
    }

    fn send_executor_event(&self, event: ExecutorEvent) -> Result<(), String> {
        self.state
            .access
            .session_tx
            .send(SessionEvent::Executor(event))
            .map_err(|_| "executor event send failed: session stopped".to_owned())
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
