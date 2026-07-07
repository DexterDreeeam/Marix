use crate::executor::Tool;
use marix_common::SharedNetSender;
use marix_protocol::{ExecutionRequest, SessionMessage};

pub struct ExecutionState {
    pub tool: Tool,
    pub parameters: ExecutionRequest,
    pub server_tx: SharedNetSender<SessionMessage>,
}

impl ExecutionState {
    pub fn new(
        tool: Tool,
        parameters: ExecutionRequest,
        server_tx: SharedNetSender<SessionMessage>,
    ) -> Self {
        Self {
            tool,
            parameters,
            server_tx,
        }
    }
}
