use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;

use crate::agent::frontdoor::Task;
use crate::agent::model::{
    DeepseekBackend, ModelBackend, ModelBackendError, ModelRequest, ModelResponse,
};
use crate::common::channel::SessionTaskId;
use crate::common::config::{Config, ModelBackend as ConfigModelBackend};
use crate::common::message::ChatResponseSegment;
use crate::common::message::RequestMessageEnvelope;

use super::{
    LoopEngineError, ModelTaskStepKind, SessionContext, TaskContext, TaskRuntimeEvent, TaskStatus,
    TaskStep, TaskStepKind,
};

#[derive(Clone)]
pub(crate) struct LoopEngine {
    session: SessionContext,
    backend: Arc<Mutex<Box<dyn ModelBackend>>>,
    task_contexts: Arc<Mutex<HashMap<SessionTaskId, TaskContext>>>,
}

impl LoopEngine {
    pub(crate) fn new() -> Result<Self, LoopEngineError> {
        let config = Config::load().map_err(LoopEngineError::BackendUnavailable)?;
        Ok(Self {
            session: SessionContext::new(),
            backend: Arc::new(Mutex::new(Self::build_model_backend(&config))),
            task_contexts: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub(crate) fn session_context(&self) -> &SessionContext {
        &self.session
    }

    pub(crate) fn create_task_context(
        &self,
        mut task: Task,
    ) -> Result<TaskContext, LoopEngineError> {
        let task_id = task.task_id();
        let initial_message = Arc::new(
            task.take_initial_request()
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
            .spawn(move || {
                engine.publish_status(&context, TaskStatus::Running);
                let status = match engine.run_model_loop(&context) {
                    Ok(()) => match engine.complete_client_task(&context) {
                        Ok(()) => TaskStatus::Succeeded,
                        Err(_) => TaskStatus::Stopped,
                    },
                    Err(status) => {
                        let _ = engine.complete_client_task(&context);
                        status
                    }
                };
                engine.publish_status(&context, status);
            })
            .map_err(|error| LoopEngineError::TaskFailed(error.to_string()))?;
        Ok(runtime_rx)
    }
}

// -- Private -- //

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

    fn run_model_loop(
        &self,
        context: &TaskContext,
    ) -> Result<(), TaskStatus> {
        let step = self.initial_task_step(context);
        self.publish_model_request(context, step.output.clone());
        let TaskStepKind::Model(kind) = step.kind else {
            return Err(TaskStatus::Failed("task step is not a model step".to_owned()));
        };
        let request = ModelRequest {
            step: kind,
            prompt: step.output.clone(),
        };
        let mut backend = self
            .backend
            .lock()
            .map_err(|_| TaskStatus::Failed("model backend is poisoned".to_owned()))?;
        self.stream_model_response(backend.as_mut(), request, context)
    }

    fn initial_task_step(&self, context: &TaskContext) -> TaskStep {
        TaskStep {
            sequence: 0,
            kind: TaskStepKind::Model(ModelTaskStepKind::Initial),
            output: self.prompt_from_message(context.initial_message()),
        }
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

    fn prompt_from_message(&self, message: &RequestMessageEnvelope) -> String {
        match message {
            RequestMessageEnvelope::ChatRequest(chat) => chat.content.clone(),
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
        self.task(context).send(ChatResponseSegment { content })
    }

    fn complete_client_task(
        &self,
        context: &TaskContext,
    ) -> Result<(), crate::common::channel::ChannelError> {
        self.task(context).complete()
    }

    fn task<'a>(&self, context: &'a TaskContext) -> std::sync::MutexGuard<'a, Task> {
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

    fn build_model_backend(config: &Config) -> Box<dyn ModelBackend> {
        match config.model.backend {
            ConfigModelBackend::Deepseek => {
                Box::new(DeepseekBackend::new(config.model.deepseek.clone()))
            }
        }
    }
}
