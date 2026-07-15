use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::external::*;
use marix_common::{AsyncReceiver, AsyncSender, Logger, build_async_channel};
use marix_protocol::{
    IntentEvent, InvocationEvent, InvocationSignature,
    InvocationStatus, SessionEvent, StepEvent, StepResult,
    StepResultKind, StepStatus, TaskEvent,
};

use super::StepState;
use crate::task::TaskAccess;

pub struct StepRuntime {
    pub access: Arc<TaskAccess>,
    pub state: Arc<StepState>,
    pub step_rx: StdMutex<Option<AsyncReceiver<StepEvent>>>,
    pub close_tx: AsyncSender<()>,
    pub close_rx: StdMutex<Option<AsyncReceiver<()>>>,
}

impl StepRuntime {
    pub async fn run(&self) {
        let Some(mut step_rx) = self
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
        if self.state.invocations.list().is_empty() {
            self.fail("step has no invocations".to_owned());
            return;
        }
        self.set_status(StepStatus::Running);
        for invocation in self.state.invocations.working_list() {
            invocation.start();
        }

        let Some(mut close_rx) = self
            .close_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
        else {
            self.fail("step close receiver unavailable".to_owned());
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
    pub(crate) fn new(access: Arc<TaskAccess>, state: Arc<StepState>) -> Self {
        let (close_tx, close_rx) = build_async_channel();
        let step_rx = state
            .step_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
        Self {
            access,
            state,
            step_rx: StdMutex::new(step_rx),
            close_tx,
            close_rx: StdMutex::new(Some(close_rx)),
        }
    }

    fn on_invocation_update(&self, signature: InvocationSignature, status: InvocationStatus) {
        if self.status().is_terminal() {
            Logger::error(format!(
                "step {} received invocation {signature} update {status:?} \
                 after completion",
                &self.state.signature,
            ));
            return;
        }
        if !status.is_terminal() {
            return;
        }
        let Some(invocation) = self.state.invocations.with(&signature, Clone::clone) else {
            self.fail(format!("invocation {signature} not found"));
            return;
        };
        match status {
            InvocationStatus::Succeed { .. } => {
                if invocation.result().is_none() {
                    self.fail(format!("invocation {signature} succeeded without output"));
                    return;
                }
                if !self.complete_invocation(&signature) {
                    Logger::error(format!(
                        "step {} received duplicate complete update from \
                         invocation {signature}",
                        &self.state.signature,
                    ));
                    return;
                }
                if self.state.invocations.working_size() == 0 {
                    self.succeed();
                }
            }
            InvocationStatus::Failed => {
                if !self.complete_invocation(&signature) {
                    Logger::error(format!(
                        "step {} received duplicate complete update from \
                         invocation {signature}",
                        &self.state.signature,
                    ));
                    return;
                }
                self.fail(format!(
                    "invocation {signature} failed at the system boundary"
                ));
            }
            InvocationStatus::Canceled => {
                if !self.complete_invocation(&signature) {
                    Logger::error(format!(
                        "step {} received duplicate complete update from \
                         invocation {signature}",
                        &self.state.signature,
                    ));
                    return;
                }
                self.cancel();
            }
            InvocationStatus::Created | InvocationStatus::Started => {}
        }
    }

    pub(crate) fn succeed(&self) {
        let invocations = self.state.invocations.list();
        let mut outputs = Vec::with_capacity(invocations.len());
        for invocation in invocations {
            let signature = &invocation.state.signature;
            let Some(output) = invocation.result() else {
                self.fail(format!("invocation {signature} succeeded without output"));
                return;
            };
            outputs.push(format!("{}: {output}", &signature.name));
        }
        let result = StepResult {
            kind: StepResultKind::Succeed,
            output: outputs.join("\n"),
        };
        self.finish(result);
    }

    fn fail(&self, reason: String) {
        Logger::error(format!("step {} failed: {reason}", &self.state.signature,));
        self.finish(StepResult {
            kind: StepResultKind::Failed,
            output: reason,
        });
    }

    fn cancel(&self) {
        if self.status().is_terminal() {
            return;
        }
        for invocation in self.state.invocations.working_list() {
            if !invocation.status().is_terminal() {
                let invocation_signature =
                    invocation.state.signature.clone();
                let event = SessionEvent::Task(
                    self.access.signature.clone(),
                    TaskEvent::Invocation(
                        invocation_signature.clone(),
                        InvocationEvent::Cancel,
                    ),
                );
                if self.access.session_tx.send(event).is_err() {
                    Logger::warning(format!(
                        "step {} invocation {invocation_signature} cancel \
                         failed: session stopped",
                        &self.state.signature,
                    ));
                }
            }
        }
        self.finish(StepResult {
            kind: StepResultKind::Canceled,
            output: "step canceled".to_owned(),
        });
    }

    fn finish(&self, result: StepResult) {
        let status = StepStatus::Complete(result);
        let mut current = self
            .state
            .status
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if current.is_terminal() {
            return;
        }
        *current = status.clone();
        drop(current);
        self.send_intent_update(status);
        self.close();
    }

    fn complete_invocation(&self, signature: &InvocationSignature) -> bool {
        let is_working = self
            .state
            .invocations
            .working_list()
            .iter()
            .any(|invocation| &invocation.state.signature == signature);
        if is_working {
            self.state.invocations.complete(signature.clone());
        }
        is_working
    }

    fn status(&self) -> StepStatus {
        self.state
            .status
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    fn set_status(&self, status: StepStatus) {
        *self
            .state
            .status
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = status;
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
        if self.access.session_tx.send(event).is_err() {
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
