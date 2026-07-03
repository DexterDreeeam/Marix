use marix_common::{ExecutionParameterPackage, SessionEvent, SharedNetSender};
use marix_host_tool::Tool;

pub struct ExecutionContext {
    pub tool: Tool,
    pub parameters: ExecutionParameterPackage,
    pub agent_tx: SharedNetSender<SessionEvent>,
}

impl ExecutionContext {
    pub fn new(
        tool: Tool,
        parameters: ExecutionParameterPackage,
        agent_tx: SharedNetSender<SessionEvent>,
    ) -> Self {
        Self {
            tool,
            parameters,
            agent_tx,
        }
    }
}
