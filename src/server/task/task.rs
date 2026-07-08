use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::thread::{self, JoinHandle};

use marix_common::{
    Config, Logger, ModelBackend as ConfigModelBackend, Receiver, Sender, build_channel,
};
use marix_protocol::{
    SessionEvent, TaskEvent, TaskPreview, TaskRequestBrief, TaskResult, TaskSignature, TaskStatus,
};

use crate::model::{DeepseekBackend, ModelBackend};
use crate::session::SessionContext;
use crate::step::Step;
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
            task_tx.clone(),
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
        let _ = Logger::log(format!("task {} started", state.signature.id.0));
        Step::trigger_initial_plan(Arc::clone(&state));
        while let Ok(event) = task_rx.recv() {
            match event {
                SessionEvent::Task(_, TaskEvent::Cancel) => {
                    let _ = Logger::log(format!("task {} canceled", state.signature.id.0));
                    Self::send_status_event(&state, TaskStatus::Canceled);
                    break;
                }
                SessionEvent::Task(_, TaskEvent::Status(TaskStatus::Succeed(result))) => {
                    let _ = Logger::log(format!("task {} succeeded", state.signature.id.0));
                    Self::send_status_event(&state, TaskStatus::Succeed(result));
                    break;
                }
                SessionEvent::Task(_, event) => {
                    Self::route_task_event(&state, event);
                }
                SessionEvent::Step(signature, event) => {
                    Step::route_step_event(Arc::clone(&state), signature, event);
                }
                SessionEvent::Execution(signature, event) => {
                    state.execution_hub.route_event(&state, signature, event);
                }
                SessionEvent::Relay(signature, event) => {
                    state.relay_hub.route_event(&state, signature, event);
                }
                SessionEvent::Plan(signature, event) => {
                    state.plan_hub.route_event(&state, signature, event);
                }
            }
        }
    }

    fn route_task_event(_state: &TaskState, event: TaskEvent) {
        match event {
            // Remaining TaskEvents (Create / CreateFailed / Query / Preview / non-Succeed
            // Status); the worker has no handling for these yet, placeholder to be filled in.
            TaskEvent::Create { .. }
            | TaskEvent::CreateFailed { .. }
            | TaskEvent::Query
            | TaskEvent::Preview { .. }
            | TaskEvent::Cancel
            | TaskEvent::Status(_) => {}
        }
    }

    fn send_status_event(state: &TaskState, status: TaskStatus) {
        let event = SessionEvent::Task(state.signature.clone(), TaskEvent::Status(status));
        let _ = state.session_tx.send(event);
    }
}
