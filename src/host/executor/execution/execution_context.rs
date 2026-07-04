use crate::executor::Tool;
use marix_common::{ExecutionRequest, SessionMessage, SharedNetSender};

pub struct ExecutionContext {
    pub tool: Tool,
    pub parameters: ExecutionRequest,
    pub agent_tx: SharedNetSender<SessionMessage>,
}

impl ExecutionContext {
    pub fn new(
        tool: Tool,
        parameters: ExecutionRequest,
        agent_tx: SharedNetSender<SessionMessage>,
    ) -> Self {
        Self {
            tool,
            parameters,
            agent_tx,
        }
    }
}
