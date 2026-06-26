use std::sync::mpsc;

use crate::agent::frontdoor::AgentTask;
use crate::agent::model::ModelBackendType;

use super::{LoopEngineError, SessionContext, TaskContext, TaskRuntimeEvent};

pub(crate) struct LoopEngine {
    session: SessionContext,
    backend: ModelBackendType,
}

impl LoopEngine {
    pub(crate) fn new(backend: ModelBackendType) -> Result<Self, LoopEngineError> {
        panic!("not implemented")
    }

    pub(crate) fn session_context(&self) -> &SessionContext {
        panic!("not implemented")
    }

    pub(crate) fn create_task_context(
        &self,
        task: AgentTask,
    ) -> Result<TaskContext, LoopEngineError> {
        panic!("not implemented")
    }

    pub(crate) fn run_task(
        &self,
        context: TaskContext,
    ) -> Result<mpsc::Receiver<TaskRuntimeEvent>, LoopEngineError> {
        panic!("not implemented")
    }
}
