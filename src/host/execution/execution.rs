use std::sync::Arc;
use std::thread;

use crate::executor::Tool;
use marix_common::{Logger, SharedNetSender};
use marix_protocol::{Actor, ExecutionEvent, ExecutionRequest, Runtime, SessionMessage};

use super::{ExecutionRuntime, ExecutionState};

pub struct Execution {
    state: Arc<ExecutionState>,
}

impl Execution {
    pub fn new(
        tool: Tool,
        request: ExecutionRequest,
        server_tx: SharedNetSender<SessionMessage>,
    ) -> Self {
        Self {
            state: Arc::new(ExecutionState::new(tool, request, server_tx)),
        }
    }
}

impl Actor<Execution, ExecutionEvent> for Execution {
    fn start(&mut self) {
        let state = Arc::clone(&self.state);
        drop(thread::spawn(move || {
            let runtime = ExecutionRuntime::new(state);
            runtime.run();
        }));
    }

    fn dispatch(&self, event: ExecutionEvent) {
        if self.state.execution_tx.send(event).is_err() {
            Logger::warning(format!(
                "execution {} event dispatch failed: worker stopped",
                &self.state.request.signature,
            ));
        }
    }
}
