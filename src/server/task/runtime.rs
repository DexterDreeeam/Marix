use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::external::*;
use marix_common::{AsyncReceiver, AsyncSender, Logger, build_async_channel};
use marix_protocol::{
    IntentEvent, IntentResult, IntentResultKind, IntentSignature,
    IntentStatus, SessionEvent, TaskResult, TaskStatus,
};

use super::TaskState;
use crate::intent::Intent;

pub struct TaskRuntime {
    pub state: Arc<TaskState>,
    pub close_tx: AsyncSender<()>,
    pub close_rx: StdMutex<Option<AsyncReceiver<()>>>,
}

impl TaskRuntime {
    pub async fn run(&self) {
        let Some(mut task_rx) = self
            .state
            .task_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
        else {
            Logger::warning(format!(
                "task {} start ignored: already running",
                &self.state.access.signature,
            ));
            return;
        };
        self.set_status(TaskStatus::Started);
        Logger::log(format!("task {} started", &self.state.access.signature));
        let root = Intent::new(
            Arc::clone(&self.state.access),
            self.state.root.clone(),
            self.state.access.user_request.clone(),
        );
        if !self.state.access.insert_intent(root.clone()) {
            self.fail_task("root intent already exists".to_owned());
            return;
        }
        root.start();

        let Some(mut close_rx) = self
            .close_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
        else {
            self.fail_task("task close receiver unavailable".to_owned());
            return;
        };

        loop {
            self::tokio::select! {
                _ = close_rx.recv() => break,
                event = task_rx.recv() => {
                    let Some(event) = event else {
                        break;
                    };
                    self.dispatch(event);
                }
            }
        }
        Logger::debug(format!(
            "task {} runtime loop stopped",
            &self.state.access.signature,
        ));
    }
}

// -- Private -- //

impl TaskRuntime {
    pub(crate) fn new(state: Arc<TaskState>) -> Self {
        let (close_tx, close_rx) = build_async_channel();
        Self {
            state,
            close_tx,
            close_rx: StdMutex::new(Some(close_rx)),
        }
    }

    pub(super) fn on_root_update(
        &self,
        _signature: IntentSignature,
        status: IntentStatus,
    ) {
        let IntentStatus::Complete(result) = status else {
            return;
        };
        self.finish_root(result);
    }

    fn finish_root(&self, result: IntentResult) {
        let IntentResult { kind, output } = result;
        let status = match kind {
            IntentResultKind::Succeed => TaskStatus::Succeed(TaskResult {
                content: output,
            }),
            IntentResultKind::Canceled => TaskStatus::Canceled,
            IntentResultKind::Infeasible => TaskStatus::Failed {
                reason: format!("root intent infeasible: {output}"),
            },
            IntentResultKind::Failed => TaskStatus::Failed {
                reason: output,
            },
        };
        if let TaskStatus::Succeed(task_result) = &status {
            *self
                .state
                .result
                .lock()
                .unwrap_or_else(|error| error.into_inner()) = Some(task_result.clone());
        }
        self.set_status(status);
        self.close();
    }

    pub(super) fn cancel_task(&self) {
        if matches!(
            self.state
                .status
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .clone(),
            TaskStatus::Canceled | TaskStatus::Succeed(_) | TaskStatus::Failed { .. }
        ) {
            return;
        }
        for intent in self.state.intents.list() {
            if !intent.status().is_terminal() {
                intent.dispatch(IntentEvent::Cancel);
            }
        }
        self.set_status(TaskStatus::Canceled);
        self.close();
    }

    pub(super) fn fail_task(&self, reason: String) {
        Logger::error(format!(
            "task {} failed: {reason}",
            &self.state.access.signature,
        ));
        self.set_status(TaskStatus::Failed { reason });
        self.close();
    }

    fn set_status(&self, status: TaskStatus) {
        let terminal_already = matches!(
            self.state
                .status
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .clone(),
            TaskStatus::Canceled | TaskStatus::Succeed(_) | TaskStatus::Failed { .. }
        );
        if terminal_already {
            return;
        }
        *self
            .state
            .status
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = status.clone();
        if self
            .state
            .access
            .session_tx
            .send(SessionEvent::TaskUpdate(status))
            .is_err()
        {
            Logger::warning(format!(
                "task {} status update failed: session stopped",
                &self.state.access.signature,
            ));
        }
    }

    fn close(&self) {
        if self.close_tx.send(()).is_err() {
            Logger::warning(format!(
                "task {} close signal failed",
                &self.state.access.signature,
            ));
        }
    }
}
