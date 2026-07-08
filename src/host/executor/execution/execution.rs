use std::sync::Arc;
use std::thread::{self, JoinHandle};

use crate::executor::Tool;
use marix_common::{Logger, Receiver, Sender, build_channel};
use marix_protocol::{
    ExecutionError, ExecutionEvent, ExecutionRequest, ExecutionStatus, ExecutorEvent,
};

use super::ExecutionState;

pub struct Execution {
    execution_tx: Sender<ExecutionEvent>,
    _worker: JoinHandle<()>,
}

impl Execution {
    pub fn new(tool: Tool, request: ExecutionRequest, executor_tx: Sender<ExecutorEvent>) -> Self {
        let (execution_tx, execution_rx) = build_channel();
        let state = Arc::new(ExecutionState::new(tool, request, executor_tx));
        let worker = thread::spawn({
            let state = Arc::clone(&state);
            move || Self::worker(state, execution_rx)
        });
        Self {
            execution_tx,
            _worker: worker,
        }
    }

    pub fn sender(&self) -> Sender<ExecutionEvent> {
        self.execution_tx.clone()
    }
}

// -- Private -- //

impl Execution {
    fn worker(state: Arc<ExecutionState>, execution_rx: Receiver<ExecutionEvent>) {
        Self::run(&state);
        while let Ok(event) = execution_rx.recv() {
            if let Err(error) = Self::dispatch(&state, event) {
                let _ = Logger::debug(format!(
                    "execution {} worker stopping: {error:?}",
                    state.request.signature.execution_id.0
                ));
                break;
            }
        }
    }

    fn run(state: &ExecutionState) {
        Self::send_status(state, ExecutionStatus::Started);
        let content = state.tool.execute(&state.request.input.content);
        Self::send_status(state, ExecutionStatus::Processing { seq: 0, content });
        Self::send_status(state, ExecutionStatus::Succeed { seq_count: 1 });
    }

    fn dispatch(state: &ExecutionState, event: ExecutionEvent) -> Result<(), ExecutionError> {
        match event {
            ExecutionEvent::Cancel => {
                Self::on_cancel(state);
                Err(ExecutionError::Canceled)
            }
        }
    }

    fn on_cancel(state: &ExecutionState) {
        let _ = Logger::log(format!("execution canceled for tool {}", state.tool.name()));
        Self::send_status(state, ExecutionStatus::Canceled);
    }

    fn send_status(state: &ExecutionState, status: ExecutionStatus) {
        if state
            .executor_tx
            .send(ExecutorEvent::ExecutionUpdate(
                state.request.signature.clone(),
                status,
            ))
            .is_err()
        {
            let _ = Logger::warning(format!(
                "execution {} status update failed: executor worker stopped",
                state.request.signature.execution_id.0
            ));
        }
    }
}
