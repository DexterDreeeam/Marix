use std::sync::Arc;
use std::thread::{self, JoinHandle};

use marix_common::{
    Config, ExecutionParameterPackage, ExecutionSessionEvent, ModelBackend as ConfigModelBackend,
    Receiver, Sender, SessionEvent, SharedNetSender, TaskSessionEvent, TaskSignature, TaskStatus,
    build_channel,
};

use crate::model::{DeepseekBackend, ModelBackend, ModelRequest, ModelResponse};
use crate::task::{ModelStepKind, Step, StepKind, StepSequence, TaskContext};

pub struct Task {
    context: Arc<TaskContext>,
    task_tx: Sender<SessionEvent>,
    task_rx: Option<Receiver<SessionEvent>>,
    worker: Option<JoinHandle<()>>,
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
        Self {
            context,
            task_tx,
            task_rx: Some(task_rx),
            worker: None,
        }
    }

    pub fn sender(&self) -> Sender<SessionEvent> {
        self.task_tx.clone()
    }

    pub fn run(&mut self) {
        if self.worker.is_some() {
            return;
        }
        let Some(task_rx) = self.task_rx.take() else {
            panic!("task receiver is missing")
        };
        let context = Arc::clone(&self.context);
        self.worker = Some(thread::spawn(move || Self::event_loop(context, task_rx)));
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
                SessionEvent::Task(_, TaskSessionEvent::Cancel) => {
                    Self::emit_status(&context, TaskStatus::Canceled);
                    break;
                }
                SessionEvent::Execution(_, ExecutionSessionEvent::Update(_))
                | SessionEvent::Execution(_, ExecutionSessionEvent::Status(_)) => {
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
            parameters: ExecutionParameterPackage {
                task_id: context.signature.id.clone(),
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
        let event = SessionEvent::Task(context.signature.clone(), TaskSessionEvent::Status(status));
        Self::send_event(context, event);
    }

    fn send_event(context: &TaskContext, event: SessionEvent) {
        context.runtime.block_on(async {
            if let Some(net_sender) = context.client_tx.lock().await.as_mut() {
                let _ = net_sender.send(event).await;
            }
        });
    }
}
