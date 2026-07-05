use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::thread::{self, JoinHandle};

use marix_common::{Config, ModelBackend as ConfigModelBackend, Receiver, Sender, build_channel};
use marix_protocol::{
    SessionEvent, TaskEvent, TaskPreview, TaskRequestBrief, TaskResult, TaskSignature, TaskStatus,
};

use crate::model::{DeepseekBackend, ModelBackend};
use crate::session::SessionContext;
use crate::task::TaskState;

pub struct Task {
    state: Arc<TaskState>,
    task_tx: Sender<SessionEvent>,
    worker: JoinHandle<()>,
}

impl Task {
    pub fn new(
        session_context: Arc<StdMutex<SessionContext>>,
        signature: TaskSignature,
        user_request: String,
        session_tx: Sender<SessionEvent>,
    ) -> Self {
        let (task_tx, task_rx) = build_channel();
        let state = Arc::new(TaskState::new(
            session_context,
            signature,
            user_request,
            Self::build_model_backend(),
            session_tx,
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

    pub fn preview(&self) -> TaskPreview {
        TaskPreview {
            request: TaskRequestBrief {
                content: self.state.user_request.clone(),
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
        Self::send_status_event(&state, TaskStatus::Started);
        Self::trigger_initial_plan(Arc::clone(&state));
        while let Ok(event) = task_rx.recv() {
            match event {
                SessionEvent::Task(_, _) => {
                    if !Self::route_task_event(&state, event) {
                        break;
                    }
                }
                SessionEvent::Step(signature, event) => {
                    if !Self::route_step_event(Arc::clone(&state), signature, event) {
                        break;
                    }
                }
                SessionEvent::Execution(_, event) => {
                    Self::route_execution_session_event(&state, event)
                }
            }
        }
    }

    fn route_task_event(state: &TaskState, event: SessionEvent) -> bool {
        match event {
            SessionEvent::Task(_, TaskEvent::Cancel) => {
                Self::send_status_event(state, TaskStatus::Canceled);
                false
            }
            SessionEvent::Task(_, TaskEvent::Status(TaskStatus::Succeed(_))) => false,
            _ => true,
        }
    }

    fn send_status_event(state: &TaskState, status: TaskStatus) {
        let event = SessionEvent::Task(state.signature.clone(), TaskEvent::Status(status));
        let _ = state.session_tx.send(event);
    }
}
