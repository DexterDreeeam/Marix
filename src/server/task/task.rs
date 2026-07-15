use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::thread;

use marix_common::{Logger, Sender};
use marix_protocol::{
    IntentSignature, SessionEvent, TaskEvent, TaskResult, TaskSignature, TaskStatus,
};

use super::{TaskRuntime, TaskState};
use crate::session::SessionContext;

#[derive(Clone)]
pub struct Task {
    pub state: Arc<TaskState>,
}

impl Task {
    pub fn status(&self) -> TaskStatus {
        self.state
            .status
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    pub fn result(&self) -> Option<TaskResult> {
        self.state
            .result
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    pub fn start(&self) {
        let runtime = TaskRuntime::new(Arc::clone(&self.state));
        let rt = Arc::clone(&self.state.access.rt);
        drop(thread::spawn(move || {
            rt.block_on(runtime.run());
        }));
    }

    pub fn dispatch(&self, event: TaskEvent) {
        if self.state.task_tx.send(event).is_err() {
            Logger::warning(format!(
                "task {} event dispatch failed: worker stopped",
                &self.state.access.signature,
            ));
        }
    }
}

// -- Private -- //

impl Task {
    pub(crate) fn new(
        session_context: Arc<StdMutex<SessionContext>>,
        signature: TaskSignature,
        root: IntentSignature,
        user_request: String,
        session_tx: Sender<SessionEvent>,
    ) -> Self {
        let state = Arc::new(TaskState::new(
            session_context,
            signature,
            root,
            user_request,
            session_tx,
        ));
        Self { state }
    }
}
