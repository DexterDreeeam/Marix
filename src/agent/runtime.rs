use super::types::{AgentContext, AgentId, AgentInput, AgentOutput, AgentResult};

/// Common interface for all Marix agents.
pub trait Agent {
    fn id(&self) -> &AgentId;

    fn run(&self, input: AgentInput, context: AgentContext) -> AgentResult<AgentOutput>;
}
