use std::sync::Arc;
use std::thread::{self, JoinHandle};

use marix_common::{
    Config, ExeId, ExecutionEvent, ExecutionRequest, ExecutionSignature,
    ModelBackend as ConfigModelBackend, Receiver, Sender, SessionEvent, SharedNetSender, TaskEvent,
    TaskSignature, TaskStatus, build_channel,
};

use crate::model::{DeepseekBackend, ModelBackend, ModelRequest, ModelResponse};
use crate::task::{ModelStepKind, Step, StepKind, StepSequence, TaskContext};

pub struct Task {
    context: Arc<TaskContext>,
    task_tx: Sender<SessionEvent>,
    worker: JoinHandle<()>,
}

impl Task {
    pub fn new(
        signature: TaskSignature,
        client_tx: SharedNetSender<SessionEvent>,
        host_tx: SharedNetSender<SessionEvent>,
    ) -> Self {
        let (task_tx, task_rx) = build_channel();
        let context = Arc::new(TaskContext::new(
            signature,
            Self::build_model_backend(),
            client_tx,
            host_tx,
        ));
        let worker = thread::spawn({
            let context = Arc::clone(&context);
            move || Self::event_loop(context, task_rx)
        });
        Self {
            context,
            task_tx,
            worker,
        }
    }

    pub fn sender(&self) -> Sender<SessionEvent> {
        self.task_tx.clone()
    }

    pub fn raise(&self, step: Step) {
        Self::run_step(Arc::clone(&self.context), step);
    }
}

// -- Private -- //

impl Task {
    fn build_model_backend() -> Box<dyn ModelBackend> {
        let config =
            Config::load().unwrap_or_else(|error| panic!("failed to load config: {error}"));
        match config.model.backend {
            ConfigModelBackend::Deepseek => Box::new(DeepseekBackend::new()),
        }
    }

    fn event_loop(context: Arc<TaskContext>, task_rx: Receiver<SessionEvent>) {
        Self::emit_status(&context, TaskStatus::Started);
        Self::run_step(Arc::clone(&context), Self::initial_step(&context));
        while let Ok(event) = task_rx.recv() {
            match event {
                SessionEvent::Task(_, TaskEvent::Cancel) => {
                    Self::emit_status(&context, TaskStatus::Canceled);
                    break;
                }
                SessionEvent::Execution(_, ExecutionEvent::Update(_))
                | SessionEvent::Execution(_, ExecutionEvent::Status(_)) => {
                    Self::send_event(&context, event);
                }
                _ => {}
            }
        }
    }

    fn initial_step(context: &TaskContext) -> Step {
        Step {
            sequence: StepSequence(0),
            kind: StepKind::Model(ModelStepKind::Initial),
            parameters: ExecutionRequest {
                signature: ExecutionSignature {
                    task_id: context.signature.id.clone(),
                    exe_id: ExeId::new(),
                    name: context.signature.name.clone(),
                },
                prompt: Some(context.signature.name.clone()),
                tool_request: None,
                user_options: Vec::new(),
            },
        }
    }

    fn run_step(context: Arc<TaskContext>, step: Step) {
        context.steps.insert_or_update(step.sequence, step.clone());
        thread::spawn(move || {
            if matches!(step.kind, StepKind::Model(_)) {
                Self::execute_model_step(&context, step);
            } else {
                Self::emit_status(
                    &context,
                    TaskStatus::Failed {
                        reason: "task step kind is not supported yet".to_owned(),
                    },
                );
            }
        });
    }

    fn execute_model_step(context: &TaskContext, step: Step) {
        let prompt = step
            .parameters
            .prompt
            .clone()
            .unwrap_or_else(|| context.signature.name.clone());
        let request = ModelRequest { step, prompt };
        let responses = {
            let mut backend = context
                .model_backend
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            backend.request(request)
        };
        let responses = match responses {
            Ok(responses) => responses,
            Err(error) => {
                Self::emit_status(
                    context,
                    TaskStatus::Failed {
                        reason: error.to_string(),
                    },
                );
                return;
            }
        };
        for response in responses {
            match response {
                ModelResponse::Content(content) => {
                    Self::emit_status(context, TaskStatus::Update { content });
                }
                ModelResponse::Failed(error) => {
                    Self::emit_status(
                        context,
                        TaskStatus::Failed {
                            reason: error.to_string(),
                        },
                    );
                    return;
                }
            }
        }
        Self::emit_status(context, TaskStatus::Succeed);
    }

    fn emit_status(context: &TaskContext, status: TaskStatus) {
        let event = SessionEvent::Task(context.signature.clone(), TaskEvent::Status(status));
        Self::send_event(context, event);
    }

    fn send_event(context: &TaskContext, event: SessionEvent) {
        if let Some(sender) = context
            .client_tx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_mut()
        {
            let _ = sender.try_send(event);
        }
    }
}
