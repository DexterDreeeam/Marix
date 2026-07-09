use std::sync::Arc;
use std::thread;

use marix_common::{Logger, SharedNetSender};
use marix_protocol::{Actor, ExecutorEvent, Runtime, SessionMessage, ToolPreview};

use super::runtime::ExecutorRuntime;
use super::state::ExecutorState;

pub struct Executor {
    state: Arc<ExecutorState>,
}

impl Executor {
    pub fn new(server_tx: SharedNetSender<SessionMessage>) -> Self {
        Self {
            state: Arc::new(ExecutorState::new(server_tx)),
        }
    }

    pub fn preview(&self) -> Vec<ToolPreview> {
        self.state.registry.preview()
    }
}

impl Actor<Executor, ExecutorEvent> for Executor {
    fn start(&mut self) {
        let state = Arc::clone(&self.state);
        drop(thread::spawn(move || {
            let runtime = ExecutorRuntime::new(state);
            runtime.run();
        }));
    }

    fn dispatch(&self, event: ExecutorEvent) {
        if self.state.executor_tx.send(event).is_err() {
            Logger::warning("host executor event dispatch failed: worker stopped");
        }
    }
}
