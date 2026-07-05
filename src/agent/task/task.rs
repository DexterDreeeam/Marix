use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::{self, JoinHandle};

use marix_common::{
    Config, ModelBackend as ConfigModelBackend, Receiver, Sender, SharedNetSender, build_channel,
};
use marix_protocol::{
    ExecutionEvent, ModelStepKind, SessionEvent, SessionMessage, StepEvent, StepKind, StepResult,
    StepSignature, StepStatus, TaskEvent, TaskPreview, TaskRequestBrief, TaskResult, TaskSignature,
    TaskStatus,
};

use crate::model::{DeepseekBackend, ModelBackend, ModelRequest, ModelResponse};
use crate::prompt::{InitialPrompt, Prompt};
use crate::session::Session;
use crate::session::SessionContext;
use crate::task::{Step, TaskState};

pub struct Task {
    state: Arc<TaskState>,
    task_tx: Sender<SessionEvent>,
    worker: JoinHandle<()>,
}

impl Task {
    pub fn new(
        session_context: Arc<StdMutex<SessionContext>>,
        signature: TaskSignature,
        client_tx: SharedNetSender<SessionMessage>,
        host_tx: SharedNetSender<SessionMessage>,
    ) -> Self {
        let (task_tx, task_rx) = build_channel();
        let state = Arc::new(TaskState::new(
            session_context,
            signature,
            Self::build_model_backend(),
            client_tx,
            host_tx,
        ));
        let worker = thread::spawn({
            let state = Arc::clone(&state);
            move || Self::run_worker(state, task_rx)
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

    pub fn preview(&self) -> TaskPreview {
        TaskPreview {
            request: TaskRequestBrief {
                content: self.state.signature.name.clone(),
            },
            result: TaskResult {
                content: String::new(),
            },
        }
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

    fn run_worker(state: Arc<TaskState>, task_rx: Receiver<SessionEvent>) {
        Self::emit_status(&state, TaskStatus::Started);
        Self::run_step(Arc::clone(&state), Self::initial_step(&state));
        while let Ok(event) = task_rx.recv() {
            match event {
                SessionEvent::Task(_, _) => {
                    if !Self::route_task_event(&state, event) {
                        break;
                    }
                }
                SessionEvent::Step(_, event) => Self::route_step_event(&state, event),
                SessionEvent::Execution(_, event) => Self::route_execution_event(&state, event),
            }
        }
    }

    fn route_task_event(state: &TaskState, event: SessionEvent) -> bool {
        match event {
            SessionEvent::Task(_, TaskEvent::Cancel) => {
                Self::emit_status(state, TaskStatus::Canceled);
                false
            }
            _ => true,
        }
    }

    fn route_step_event(_state: &TaskState, _event: StepEvent) {}

    fn route_execution_event(_state: &TaskState, _event: ExecutionEvent) {}

    fn run_step(state: Arc<TaskState>, step: Step) {
        state
            .steps
            .insert_or_update(step.signature.step_no, step.clone());
        thread::spawn(move || {
            if matches!(step.signature.kind, StepKind::Model(_)) {
                Self::execute_model_step(&state, step);
            } else {
                Self::emit_step_event(
                    &state,
                    &step.signature,
                    StepEvent::Fail {
                        result: StepResult {
                            content: "task step kind is not supported yet".to_owned(),
                        },
                    },
                );
            }
        });
    }

    fn initial_step(state: &TaskState) -> Step {
        Step {
            signature: StepSignature::new(
                state.signature.clone(),
                0,
                state.signature.name.clone(),
                StepKind::Model(ModelStepKind::Initial),
            ),
            status: StepStatus::Prepare,
            update_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn execute_model_step(state: &TaskState, step: Step) {
        Self::emit_step_event(state, &step.signature, StepEvent::Started);
        let prompt = Self::model_step_prompt(state, &step);
        let update_count = Arc::clone(&step.update_count);
        let signature = step.signature.clone();
        let mut result = String::new();
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
                Self::emit_step_event(
                    state,
                    &signature,
                    StepEvent::Fail {
                        result: StepResult {
                            content: error.to_string(),
                        },
                    },
                );
                return;
            }
        };
        for response in responses {
            match response {
                ModelResponse::Content(content) => {
                    let seq = update_count.fetch_add(1, Ordering::Relaxed);
                    result.push_str(&content);
                    Self::emit_step_event(state, &signature, StepEvent::Update { seq, content });
                }
                ModelResponse::Failed(error) => {
                    Self::emit_step_event(
                        state,
                        &signature,
                        StepEvent::Fail {
                            result: StepResult {
                                content: error.to_string(),
                            },
                        },
                    );
                    return;
                }
            }
        }
        let seq_count = update_count.load(Ordering::Relaxed);
        Self::emit_step_event(
            state,
            &signature,
            StepEvent::Complete {
                seq_count,
                result: StepResult { content: result },
            },
        );
    }

    fn emit_status(state: &TaskState, status: TaskStatus) {
        let event = SessionEvent::Task(state.signature.clone(), TaskEvent::Status(status));
        Self::send_event(state, event);
    }

    fn model_step_prompt(state: &TaskState, step: &Step) -> String {
        match step.signature.kind {
            StepKind::Model(ModelStepKind::Initial) => {
                let session_context = {
                    let context = state
                        .session_context
                        .lock()
                        .unwrap_or_else(|error| error.into_inner());
                    SessionContext {
                        system: context.system,
                        tasks: context.tasks.clone(),
                        tools: context.tools.clone(),
                    }
                };
                InitialPrompt::new(state.signature.name.clone(), session_context).prompt()
            }
            _ => state.signature.name.clone(),
        }
    }

    fn emit_step_event(state: &TaskState, signature: &StepSignature, event: StepEvent) {
        Self::send_event(state, SessionEvent::Step(signature.clone(), event));
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
