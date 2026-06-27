use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;

use crate::agent::frontdoor::AgentTask;
use crate::agent::model::{
    DeepseekBackend, ModelBackend, ModelBackendError, ModelBackendType, ModelRequest, ModelResponse,
};
use crate::common::channel::SessionTaskId;
use crate::common::config::Config;
use crate::common::message::ChatMessage;
use crate::common::message::UserMessageEnvelope;

use super::{LoopEngineError, SessionContext, TaskContext, TaskRuntimeEvent, TaskStatus};

#[derive(Clone)]
pub(crate) struct LoopEngine {
    session: SessionContext,
    backend: ModelBackendType,
    task_contexts: Arc<Mutex<HashMap<SessionTaskId, TaskContext>>>,
}

impl LoopEngine {
    pub(crate) fn new(backend: ModelBackendType) -> Result<Self, LoopEngineError> {
        Ok(Self {
            session: SessionContext::new(),
            backend,
            task_contexts: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub(crate) fn session_context(&self) -> &SessionContext {
        &self.session
    }

    pub(crate) fn create_task_context(
        &self,
        mut task: AgentTask,
    ) -> Result<TaskContext, LoopEngineError> {
        let task_id = task.task_id();
        let initial_message = Arc::new(
            task.receive()
                .map_err(|error| LoopEngineError::TaskFailed(error.to_string()))?,
        );
        let context = TaskContext {
            task_id,
            initial_message,
            task: Arc::new(Mutex::new(task)),
            status: Arc::new(Mutex::new(TaskStatus::Created)),
            runtime_tx: Arc::new(Mutex::new(None)),
        };
        let mut task_contexts = self.task_contexts.lock().map_err(|_| {
            LoopEngineError::TaskFailed("task context registry is poisoned".to_owned())
        })?;
        task_contexts.insert(context.task_id(), context.clone());
        Ok(context)
    }

    pub(crate) fn run_task(
        &self,
        context: TaskContext,
    ) -> Result<mpsc::Receiver<TaskRuntimeEvent>, LoopEngineError> {
        let (runtime_tx, runtime_rx) = mpsc::channel();
        self.attach_runtime_sender(&context, runtime_tx)?;
        let engine = self.clone();
        thread::Builder::new()
            .name(format!("marix-task-{}", context.task_id()))
            .spawn(move || engine.run_task_once(context))
            .map_err(|error| LoopEngineError::TaskFailed(error.to_string()))?;
        Ok(runtime_rx)
    }
}

impl LoopEngine {
    fn attach_runtime_sender(
        &self,
        context: &TaskContext,
        runtime_tx: mpsc::Sender<TaskRuntimeEvent>,
    ) -> Result<(), LoopEngineError> {
        let mut context_runtime_tx = context.runtime_tx.lock().map_err(|_| {
            LoopEngineError::TaskFailed("task context registry is poisoned".to_owned())
        })?;
        *context_runtime_tx = Some(runtime_tx);
        Ok(())
    }

    fn publish_status(&self, context: &TaskContext, status: TaskStatus) {
        let runtime_tx = {
            let mut context_status = self.task_status(context);
            *context_status = status.clone();
            self.runtime_tx(context)
        };
        self.publish_runtime_event(&runtime_tx, TaskRuntimeEvent::Status(status));
    }

    fn run_task_once(&self, context: TaskContext) {
        self.publish_status(&context, TaskStatus::Running);
        let status = match self.run_model_once(&context) {
            Ok(()) => match self.complete_client_task(&context) {
                Ok(()) => TaskStatus::Succeeded,
                Err(_) => TaskStatus::Stopped,
            },
            Err(status) => {
                let _ = self.complete_client_task(&context);
                status
            }
        };
        self.publish_status(&context, status);
    }

    fn run_model_once(&self, context: &TaskContext) -> Result<(), TaskStatus> {
        let prompt = self.prompt_from_message(context.initial_message());
        self.publish_model_request(context, prompt.clone());

        match self.backend {
            ModelBackendType::Deepseek => self.request_deepseek_model(prompt, context),
        }
    }

    fn request_deepseek_model(
        &self,
        prompt: String,
        context: &TaskContext,
    ) -> Result<(), TaskStatus> {
        let config = Config::load().map_err(TaskStatus::Failed)?;
        let mut backend = DeepseekBackend::new(&config.model.deepseek);
        self.stream_model_response(&mut backend, ModelRequest { prompt }, context)
    }

    fn stream_model_response(
        &self,
        backend: &mut dyn ModelBackend,
        request: ModelRequest,
        context: &TaskContext,
    ) -> Result<(), TaskStatus> {
        let responses = backend
            .request(request)
            .map_err(|error| self.task_status_from_backend_error(error))?;
        for response in responses {
            match response {
                ModelResponse::Content(content) => {
                    self.publish_model_response(context, content.clone());
                    self.send_model_content(context, content)
                        .map_err(|_| TaskStatus::Stopped)?;
                }
                ModelResponse::Failed(error) => {
                    return Err(self.task_status_from_backend_error(error));
                }
            }
        }
        Ok(())
    }

    fn prompt_from_message(&self, message: &UserMessageEnvelope) -> String {
        match message {
            UserMessageEnvelope::Chat(chat) => chat.content.clone(),
        }
    }

    fn publish_model_request(&self, context: &TaskContext, prompt: String) {
        let runtime_tx = self.runtime_tx(context);
        self.publish_runtime_event(&runtime_tx, TaskRuntimeEvent::ModelRequest(prompt));
    }

    fn publish_model_response(&self, context: &TaskContext, content: String) {
        let runtime_tx = self.runtime_tx(context);
        self.publish_runtime_event(&runtime_tx, TaskRuntimeEvent::ModelResponse(content));
    }

    fn publish_runtime_event(
        &self,
        runtime_tx: &Option<mpsc::Sender<TaskRuntimeEvent>>,
        event: TaskRuntimeEvent,
    ) {
        if let Some(runtime_tx) = runtime_tx {
            let _ = runtime_tx.send(event);
        }
    }

    fn send_model_content(
        &self,
        context: &TaskContext,
        content: String,
    ) -> Result<(), crate::common::channel::ChannelError> {
        self.task(context).send(ChatMessage { content })
    }

    fn complete_client_task(
        &self,
        context: &TaskContext,
    ) -> Result<(), crate::common::channel::ChannelError> {
        self.task(context).complete()
    }

    fn task<'a>(&self, context: &'a TaskContext) -> std::sync::MutexGuard<'a, AgentTask> {
        context
            .task
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }

    fn task_status<'a>(&self, context: &'a TaskContext) -> std::sync::MutexGuard<'a, TaskStatus> {
        context
            .status
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }

    fn runtime_tx(&self, context: &TaskContext) -> Option<mpsc::Sender<TaskRuntimeEvent>> {
        context
            .runtime_tx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    fn task_status_from_backend_error(&self, error: ModelBackendError) -> TaskStatus {
        TaskStatus::Failed(error.to_string())
    }
}
