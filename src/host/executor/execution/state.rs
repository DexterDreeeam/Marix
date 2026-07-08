use crate::executor::Tool;
use marix_common::Sender;
use marix_protocol::{ExecutionRequest, ExecutorEvent};

pub struct ExecutionState {
    pub tool: Tool,
    pub request: ExecutionRequest,
    pub executor_tx: Sender<ExecutorEvent>,
}

impl ExecutionState {
    pub fn new(tool: Tool, request: ExecutionRequest, executor_tx: Sender<ExecutorEvent>) -> Self {
        Self {
            tool,
            request,
            executor_tx,
        }
    }
}
