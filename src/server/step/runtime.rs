use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::external::*;
use marix_common::{AsyncReceiver, AsyncSender, Logger, build_async_channel};
use marix_protocol::{
    IntentEvent, InvocationEvent, InvocationResultKind, InvocationSignature, InvocationStatus,
    SessionEvent, StepEvent, StepResult, StepResultKind, StepStatus, TaskEvent,
};

use super::StepState;

pub struct StepRuntime {
    pub state: Arc<StepState>,
    pub close_tx: AsyncSender<()>,
    pub close_rx: StdMutex<Option<AsyncReceiver<()>>>,
}

impl StepRuntime {
    pub async fn run(&self) {
        let Some(mut step_rx) = self
            .state
            .step_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
        else {
            Logger::warning(format!(
                "step {} start ignored: already running",
                &self.state.signature,
            ));
            return;
        };
        self.set_status(StepStatus::Running);
        for signature in &self.state.invocations {
            if let Err(reason) = self.start_invocation(signature) {
                self.finish(StepResultKind::Failed, reason);
                return;
            }
        }

        let Some(mut close_rx) = self
            .close_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
        else {
            self.finish(
                StepResultKind::Failed,
                "step close receiver unavailable".to_owned(),
            );
            return;
        };

        loop {
            self::tokio::select! {
                _ = close_rx.recv() => break,
                event = step_rx.recv() => {
                    let Some(event) = event else {
                        break;
                    };
                    self.dispatch(event);
                }
            }
        }
    }

    pub fn dispatch(&self, event: StepEvent) {
        match event {
            StepEvent::Update(signature, status) => {
                self.on_invocation_update(signature, status);
            }
            StepEvent::Cancel => self.cancel(),
        }
    }
}

// -- Private -- //

impl StepRuntime {
    pub(crate) fn new(state: Arc<StepState>) -> Self {
        let (close_tx, close_rx) = build_async_channel();
        Self {
            state,
            close_tx,
            close_rx: StdMutex::new(Some(close_rx)),
        }
    }

    fn start_invocation(&self, signature: &InvocationSignature) -> Result<(), String> {
        let event = SessionEvent::Task(
            self.state.access.signature.clone(),
            TaskEvent::InvocationStart(signature.clone()),
        );
        self.state.access.session_tx.send(event).map_err(|_| {
            format!("invocation {signature} start failed: session stopped")
        })
    }

    fn on_invocation_update(&self, signature: InvocationSignature, status: InvocationStatus) {
        if self.status().is_terminal() {
            Logger::error(format!(
                "step {} received invocation {signature} update {status:?} after completion",
                &self.state.signature,
            ));
            return;
        }
        let InvocationStatus::Complete(result) = status else {
            return;
        };
        match result.kind {
            InvocationResultKind::Succeed => self.advance(),
            InvocationResultKind::Canceled => {
                self.finish(StepResultKind::Canceled, result.output);
            }
            InvocationResultKind::Failed => {
                self.finish(StepResultKind::Failed, result.output);
            }
        }
    }

    fn advance(&self) {
        let mut outputs = Vec::with_capacity(self.state.invocations.len());
        for signature in &self.state.invocations {
            let Some(result) = self.state.access.get_invocation_result(signature) else {
                return;
            };
            match result.kind {
                InvocationResultKind::Succeed => {
                    outputs.push(format!("{}: {}", &signature.name, result.output));
                }
                InvocationResultKind::Canceled => {
                    self.finish(StepResultKind::Canceled, result.output);
                    return;
                }
                InvocationResultKind::Failed => {
                    self.finish(StepResultKind::Failed, result.output);
                    return;
                }
            }
        }
        self.finish(StepResultKind::Succeed, outputs.join("\n"));
    }

    fn cancel(&self) {
        if self.status().is_terminal() {
            return;
        }
        for signature in &self.state.invocations {
            if self.state.access.get_invocation_result(signature).is_some() {
                continue;
            }
            let event = SessionEvent::Task(
                self.state.access.signature.clone(),
                TaskEvent::Invocation(
                    signature.clone(),
                    InvocationEvent::Cancel,
                ),
            );
            if self.state.access.session_tx.send(event).is_err() {
                Logger::warning(format!(
                    "step {} invocation {signature} cancel failed: session stopped",
                    &self.state.signature,
                ));
            }
        }
        self.finish(StepResultKind::Canceled, "step canceled".to_owned());
    }

    fn finish(&self, kind: StepResultKind, output: String) {
        let result = StepResult { kind, output };
        self.set_status(StepStatus::Complete(result));
        self.close();
    }

    fn status(&self) -> StepStatus {
        self.state
            .status
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    fn set_status(&self, status: StepStatus) {
        let mut current = self
            .state
            .status
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if current.is_terminal() {
            return;
        }
        let send_update = matches!(&status, StepStatus::Complete(_));
        *current = status.clone();
        drop(current);
        if send_update {
            self.send_intent_update(status);
        }
    }

    fn send_intent_update(&self, status: StepStatus) {
        let intent = self.state.signature.intent.clone();
        let event = SessionEvent::Task(
            intent.task.clone(),
            TaskEvent::Intent(
                intent,
                IntentEvent::StepUpdate(self.state.signature.clone(), status),
            ),
        );
        if self.state.access.session_tx.send(event).is_err() {
            Logger::warning(format!(
                "step {} update failed: session stopped",
                &self.state.signature,
            ));
        }
    }

    fn close(&self) {
        if self.close_tx.send(()).is_err() {
            Logger::warning(format!(
                "step {} close signal failed",
                &self.state.signature,
            ));
        }
    }
}
