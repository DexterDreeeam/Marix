use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::{Logger, Sender};
use marix_protocol::{
    Actor, RuntimeAsync, SessionEvent, TaskEvent, TaskPreview, TaskRequestBrief, TaskResult,
    TaskSignature,
};

use crate::session::SessionContext;
use crate::task::{TaskRuntime, TaskState};

pub struct Task {
    state: Arc<TaskState>,
}

impl Task {
    pub fn new(
        session_context: Arc<StdMutex<SessionContext>>,
        signature: TaskSignature,
        user_request: String,
        session_tx: Sender<SessionEvent>,
    ) -> Self {
        let state = Arc::new(TaskState::new(
            session_context,
            signature,
            user_request,
            session_tx,
        ));
        Self { state }
    }

    pub fn preview(&self) -> TaskPreview {
        TaskPreview {
            request: TaskRequestBrief {
                content: self.state.access.user_request.clone(),
            },
            result: TaskResult {
                content: String::new(),
            },
        }
    }
}

impl Actor<Task, TaskEvent> for Task {
    fn start(&mut self) {
        let rt = Arc::clone(&self.state.access.rt);
        let state = Arc::clone(&self.state);
        drop(rt.spawn(async move {
            let task_runtime = TaskRuntime::new(state);
            task_runtime.run().await;
        }));
    }

    fn dispatch(&self, event: TaskEvent) {
        if self.state.task_tx.send(event).is_err() {
            Logger::warning(format!(
                "task {} event dispatch failed: worker stopped",
                &self.state.access.signature,
            ));
        }
    }
}
