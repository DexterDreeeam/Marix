use crate::executor::Tool;
use marix_common::{ExecutionRequest, SessionEvent, SharedNetSender};

pub struct ExecutionContext {
    pub tool: Tool,
    pub parameters: ExecutionRequest,
    pub agent_tx: SharedNetSender<SessionEvent>,
}

impl ExecutionContext {
    pub fn new(
        tool: Tool,
        parameters: ExecutionRequest,
        agent_tx: SharedNetSender<SessionEvent>,
    ) -> Self {
        Self {
            tool,
            parameters,
            agent_tx,
        }
    }
}
