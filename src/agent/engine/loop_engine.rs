use crate::agent::frontdoor::AgentSession;
use crate::agent::model::ModelBackendType;
use crate::common::message::UserMessage;

use super::{session_context::SessionContext, LoopEngineError, TaskContext};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoopTaskOutcome {
    Completed,
    Cancelled,
}

pub struct LoopEngine {
    session: SessionContext,
    backend: ModelBackendType,
}

impl LoopEngine {
    pub fn new(
        session: AgentSession,
        backend: ModelBackendType,
    ) -> Result<Self, LoopEngineError> {
        panic!("not implemented")
    }

    pub fn create_task_context(
        &self,
        message: impl UserMessage,
    ) -> Result<TaskContext, LoopEngineError> {
        panic!("not implemented")
    }

    pub fn run_task(&self, task: TaskContext) -> Result<LoopTaskOutcome, LoopEngineError> {
        panic!("not implemented")
    }
}
