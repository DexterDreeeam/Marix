use std::sync::mpsc;
use std::thread;

use crate::agent::frontdoor::AgentTask;
use crate::agent::model::{
    DeepseekBackend, ModelBackend, ModelBackendError, ModelBackendType, ModelRequest, ModelResponse,
};
use crate::common::config::Config;
use crate::common::message::UserMessageEnvelope;

use super::{LoopEngineError, SessionContext, TaskContext, TaskRuntimeEvent, TaskStatus};

fn run_task_once(backend: ModelBackendType, mut context: TaskContext) {
    context.publish_status(TaskStatus::Running);
    let status = match run_model_once(backend, &mut context) {
        Ok(()) => match context.complete_client_task() {
            Ok(()) => TaskStatus::Succeeded,
            Err(_) => TaskStatus::Stopped,
        },
        Err(status) => {
            let _ = context.complete_client_task();
            status
        }
    };
    context.publish_status(status);
}

fn run_model_once(backend: ModelBackendType, context: &mut TaskContext) -> Result<(), TaskStatus> {
    let prompt = prompt_from_message(context.initial_message());
    context.publish_model_request(prompt.clone());

    match backend {
        ModelBackendType::Deepseek => request_deepseek_model(prompt, context),
    }
}

fn request_deepseek_model(prompt: String, context: &mut TaskContext) -> Result<(), TaskStatus> {
    let config = Config::load().map_err(TaskStatus::Failed)?;
    let mut backend = DeepseekBackend::new(&config.model.deepseek);
    stream_model_response(&mut backend, ModelRequest { prompt }, context)
}

fn stream_model_response(
    backend: &mut dyn ModelBackend,
    request: ModelRequest,
    context: &mut TaskContext,
) -> Result<(), TaskStatus> {
    let responses = backend
        .request(request)
        .map_err(task_status_from_backend_error)?;
    for response in responses {
        match response {
            ModelResponse::Content(content) => {
                context.publish_model_response(content.clone());
                context
                    .send_model_content(content)
                    .map_err(|_| TaskStatus::Stopped)?;
            }
            ModelResponse::Failed(error) => return Err(task_status_from_backend_error(error)),
        }
    }
    Ok(())
}

fn prompt_from_message(message: &UserMessageEnvelope) -> String {
    match message {
        UserMessageEnvelope::Chat(chat) => chat.content.clone(),
    }
}

fn task_status_from_backend_error(error: ModelBackendError) -> TaskStatus {
    TaskStatus::Failed(error.to_string())
}

pub(crate) struct LoopEngine {
    session: SessionContext,
    backend: ModelBackendType,
}

impl LoopEngine {
    pub(crate) fn new(backend: ModelBackendType) -> Result<Self, LoopEngineError> {
        Ok(Self {
            session: SessionContext::new(),
            backend,
        })
    }

    pub(crate) fn session_context(&self) -> &SessionContext {
        &self.session
    }

    pub(crate) fn create_task_context(
        &self,
        task: AgentTask,
    ) -> Result<TaskContext, LoopEngineError> {
        TaskContext::new(task).map_err(|error| LoopEngineError::TaskFailed(error.to_string()))
    }

    pub(crate) fn run_task(
        &self,
        mut context: TaskContext,
    ) -> Result<mpsc::Receiver<TaskRuntimeEvent>, LoopEngineError> {
        let (runtime_tx, runtime_rx) = mpsc::channel();
        context.attach_runtime_sender(runtime_tx);
        let backend = self.backend;
        thread::Builder::new()
            .name(format!("marix-task-{}", context.task_id()))
            .spawn(move || run_task_once(backend, context))
            .map_err(|error| LoopEngineError::TaskFailed(error.to_string()))?;
        Ok(runtime_rx)
    }
}
