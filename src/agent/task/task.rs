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
            move || Self::event_loop(state, task_rx)
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
        Self::run_step(Arc::clone(&self.state), step);
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

    fn event_loop(state: Arc<TaskState>, task_rx: Receiver<SessionEvent>) {
        Self::emit_status(&state, TaskStatus::Started);
        Self::run_step(Arc::clone(&state), Self::initial_step(&state));
        while let Ok(event) = task_rx.recv() {
            match event {
                SessionEvent::Task(_, TaskEvent::Cancel) => {
                    Self::emit_status(&state, TaskStatus::Canceled);
                    break;
                }
                SessionEvent::Execution(_, ExecutionEvent::Update(_))
                | SessionEvent::Execution(_, ExecutionEvent::Status(_)) => {
                    Self::send_event(&state, event);
                }
                _ => {}
            }
        }
    }

    fn run_step(state: Arc<TaskState>, step: Step) {
        state
            .steps
            .insert_or_update(step.signature.step_no, step.clone());
        thread::spawn(move || {
            if matches!(step.signature.kind, StepKind::Model(_)) {
                Self::execute_model_step(&state, step);
            } else {
                Self::emit_status(
                    &state,
                    TaskStatus::Failed {
                        reason: "task step kind is not supported yet".to_owned(),
                    },
                );
            }
        });
    }

    fn initial_step(state: &TaskState) -> Step {
        Step {
            signature: StepSignature {
                step_no: 0,
                name: state.signature.name.clone(),
                kind: StepKind::Model(ModelStepKind::Initial),
            },
            status: StepStatus::Prepare,
            update_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn execute_model_step(state: &TaskState, step: Step) {
        let prompt = state.signature.name.clone();
        let request = ModelRequest { step, prompt };
        let responses = {
            let mut backend = state
                .model_backend
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            backend.request(request)
        };
        let responses = match responses {
            Ok(responses) => responses,
            Err(error) => {
                Self::emit_status(
                    state,
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
                    Self::emit_status(state, TaskStatus::Update { content });
                }
                ModelResponse::Failed(error) => {
                    Self::emit_status(
                        state,
                        TaskStatus::Failed {
                            reason: error.to_string(),
                        },
                    );
                    return;
                }
            }
        }
        Self::emit_status(state, TaskStatus::Succeed);
    }

    fn emit_status(state: &TaskState, status: TaskStatus) {
        let event = SessionEvent::Task(state.signature.clone(), TaskEvent::Status(status));
        Self::send_event(state, event);
    }

    fn send_event(state: &TaskState, event: SessionEvent) {
        if let Some(sender) = state
            .client_tx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_mut()
        {
            let _ = sender.try_send(Session::package_message(event));
        }
    }
}
