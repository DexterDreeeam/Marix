use crate::agent::model::{ModelBackend, ModelBackendError, ModelRequest, ModelResponse};
use crate::common::message::{ChatMessage, UserMessageEnvelope};

use super::{session_context::SessionContext, LoopEngineError, TaskContext};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoopTaskOutcome {
    Completed,
    Cancelled,
}

pub struct LoopEngine {
    session: SessionContext,
}

impl LoopEngine {
    pub fn new() -> Self {
        Self {
            session: SessionContext::new(),
        }
    }

    pub(crate) fn session_context(&self) -> &SessionContext {
        &self.session
    }

    pub fn next_task(&self) -> Option<TaskContext> {
        self.session.next_task().map(TaskContext::new)
    }

    pub fn run_task(
        &self,
        mut task: TaskContext,
        backend: &mut dyn ModelBackend,
    ) -> Result<LoopTaskOutcome, LoopEngineError> {
        let prompt = match task.receive() {
            Ok(message) => extract_prompt(message),
            Err(_) => return Ok(LoopTaskOutcome::Cancelled),
        };

        let responses = backend
            .request(ModelRequest { prompt })
            .map_err(into_engine_error)?;

        for response in responses {
            match response {
                ModelResponse::Content(content) => {
                    if task.send(ChatMessage { content }).is_err() {
                        return Ok(LoopTaskOutcome::Cancelled);
                    }
                }
                ModelResponse::Failed(error) => return Err(into_engine_error(error)),
            }
        }

        match task.complete() {
            Ok(()) => Ok(LoopTaskOutcome::Completed),
            Err(_) => Ok(LoopTaskOutcome::Cancelled),
        }
    }
}

fn extract_prompt(message: UserMessageEnvelope) -> String {
    match message {
        UserMessageEnvelope::Chat(chat) => chat.content,
    }
}

fn into_engine_error(error: ModelBackendError) -> LoopEngineError {
    match error {
        ModelBackendError::Unavailable(reason) => LoopEngineError::BackendUnavailable(reason),
        other => LoopEngineError::BackendFailed(other.to_string()),
    }
}
