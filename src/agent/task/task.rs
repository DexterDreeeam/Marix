use std::sync::Arc;
use std::thread::{self, JoinHandle};

use marix_common::{
    Config, ExeId, ExecutionEvent, ExecutionRequest, ExecutionSignature,
    ModelBackend as ConfigModelBackend, Receiver, Sender, SessionEvent, SessionMessage,
    SharedNetSender, TaskEvent, TaskSignature, TaskStatus, build_channel,
};

use crate::model::{DeepseekBackend, ModelBackend, ModelRequest, ModelResponse};
use crate::session::Session;
use crate::task::{ModelStepKind, Step, StepKind, StepSequence, TaskState};

pub struct Task {
    context: Arc<TaskState>,
    task_tx: Sender<SessionEvent>,
    worker: JoinHandle<()>,
}

impl Task {
    pub fn new(
        signature: TaskSignature,
        client_tx: SharedNetSender<SessionMessage>,
        host_tx: SharedNetSender<SessionMessage>,
    ) -> Self {
        let (task_tx, task_rx) = build_channel();
        let context = Arc::new(TaskState::new(
            signature,
            Self::build_model_backend(),
            client_tx,
            host_tx,
        ));
        let worker = thread::spawn({
            let context = Arc::clone(&context);
            move || TaskState::event_loop(context, task_rx)
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
        Arc::clone(&self.context).run_step(step);
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
}

impl TaskState {
    fn event_loop(context: Arc<Self>, task_rx: Receiver<SessionEvent>) {
        context.emit_status(TaskStatus::Started);
        Arc::clone(&context).run_step(context.initial_step());
        while let Ok(event) = task_rx.recv() {
            match event {
                SessionEvent::Task(_, TaskEvent::Cancel) => {
                    context.emit_status(TaskStatus::Canceled);
                    break;
                }
                SessionEvent::Execution(_, ExecutionEvent::Update(_))
                | SessionEvent::Execution(_, ExecutionEvent::Status(_)) => {
                    context.send_event(event);
                }
                _ => {}
            }
        }
    }

    fn run_step(self: Arc<Self>, step: Step) {
        self.steps.insert_or_update(step.sequence, step.clone());
        thread::spawn(move || {
            if matches!(step.kind, StepKind::Model(_)) {
                self.execute_model_step(step);
            } else {
                self.emit_status(TaskStatus::Failed {
                    reason: "task step kind is not supported yet".to_owned(),
                });
            }
        });
    }

    fn initial_step(&self) -> Step {
        Step {
            sequence: StepSequence(0),
            kind: StepKind::Model(ModelStepKind::Initial),
            parameters: ExecutionRequest {
                signature: ExecutionSignature {
                    task_id: self.signature.id.clone(),
                    exe_id: ExeId::new(),
                    name: self.signature.name.clone(),
                },
                prompt: Some(self.signature.name.clone()),
                tool_request: None,
                user_options: Vec::new(),
            },
        }
    }

    fn execute_model_step(&self, step: Step) {
        let prompt = step
            .parameters
            .prompt
            .clone()
            .unwrap_or_else(|| self.signature.name.clone());
        let request = ModelRequest { step, prompt };
        let responses = {
            let mut backend = self
                .model_backend
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            backend.request(request)
        };
        let responses = match responses {
            Ok(responses) => responses,
            Err(error) => {
                self.emit_status(TaskStatus::Failed {
                    reason: error.to_string(),
                });
                return;
            }
        };
        for response in responses {
            match response {
                ModelResponse::Content(content) => {
                    self.emit_status(TaskStatus::Update { content });
                }
                ModelResponse::Failed(error) => {
                    self.emit_status(TaskStatus::Failed {
                        reason: error.to_string(),
                    });
                    return;
                }
            }
        }
        self.emit_status(TaskStatus::Succeed);
    }

    fn emit_status(&self, status: TaskStatus) {
        let event = SessionEvent::Task(self.signature.clone(), TaskEvent::Status(status));
        self.send_event(event);
    }

    fn send_event(&self, event: SessionEvent) {
        if let Some(sender) = self
            .client_tx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_mut()
        {
            let _ = sender.try_send(Session::package_message(event));
        }
    }
}
