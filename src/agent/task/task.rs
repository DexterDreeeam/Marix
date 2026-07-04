use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::thread::{self, JoinHandle};

use marix_common::{
    Config, ExecutionEvent, ModelBackend as ConfigModelBackend, ModelStepKind, Receiver, Sender,
    SessionEvent, SessionMessage, SharedNetSender, StepKind, StepSignature, StepStatus, TaskEvent,
    TaskSignature, TaskStatus, build_channel,
};

use crate::model::{DeepseekBackend, ModelBackend, ModelRequest, ModelResponse};
use crate::session::Session;
use crate::task::{Step, TaskState};

pub struct Task {
    state: Arc<TaskState>,
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
        let state = Arc::new(TaskState::new(
            signature,
            Self::build_model_backend(),
            client_tx,
            host_tx,
        ));
        let worker = thread::spawn({
            let state = Arc::clone(&state);
            move || TaskState::event_loop(state, task_rx)
        });
        Self {
            state,
            task_tx,
            worker,
        }
    }

    pub fn sender(&self) -> Sender<SessionEvent> {
        self.task_tx.clone()
    }

    pub fn raise(&self, step: Step) {
        Arc::clone(&self.state).run_step(step);
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
    fn event_loop(state: Arc<Self>, task_rx: Receiver<SessionEvent>) {
        state.emit_status(TaskStatus::Started);
        Arc::clone(&state).run_step(state.initial_step());
        while let Ok(event) = task_rx.recv() {
            match event {
                SessionEvent::Task(_, TaskEvent::Cancel) => {
                    state.emit_status(TaskStatus::Canceled);
                    break;
                }
                SessionEvent::Execution(_, ExecutionEvent::Update(_))
                | SessionEvent::Execution(_, ExecutionEvent::Status(_)) => {
                    state.send_event(event);
                }
                _ => {}
            }
        }
    }

    fn run_step(self: Arc<Self>, step: Step) {
        self.steps
            .insert_or_update(step.signature.step_no, step.clone());
        thread::spawn(move || {
            if matches!(step.signature.kind, StepKind::Model(_)) {
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
            signature: StepSignature {
                step_no: 0,
                name: self.signature.name.clone(),
                kind: StepKind::Model(ModelStepKind::Initial),
            },
            status: StepStatus::Prepare,
            update_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn execute_model_step(&self, step: Step) {
        let prompt = self.signature.name.clone();
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
