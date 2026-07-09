use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::thread::{self, JoinHandle};

use marix_common::{
    Config, Logger, ModelBackend as ConfigModelBackend, Receiver, Sender, build_channel,
};
use marix_protocol::{
    PlanDraft, PlanEvent, PlanSignature, PlanStatus, SessionEvent, TaskError, TaskEvent,
    TaskPreview, TaskRequestBrief, TaskResult, TaskSignature, TaskStatus,
};

use crate::model::{DeepseekBackend, ModelBackend};
use crate::plan::Plan;
use crate::session::SessionContext;
use crate::step::Step;
use crate::task::TaskState;

pub struct Task {
    state: Arc<TaskState>,
    task_tx: Sender<SessionEvent>,
    _worker: JoinHandle<()>,
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
            move || Self::worker(state, task_rx)
        });
        Self {
            state,
            task_tx,
            _worker: worker,
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

    fn worker(state: Arc<TaskState>, task_rx: Receiver<SessionEvent>) {
        Self::send_session_status(&state, TaskStatus::Created);
        Self::send_session_status(&state, TaskStatus::Started);
        Logger::log(format!("task {} started", state.signature.id.0));
        Step::trigger_initial_plan(Arc::clone(&state));
        while let Ok(event) = task_rx.recv() {
            if let Err(error) = Self::dispatch(&state, event) {
                Logger::debug(format!(
                    "task {} worker stopping: {error:?}",
                    state.signature.id.0
                ));
                break;
            }
        }
    }

    fn dispatch(state: &Arc<TaskState>, event: SessionEvent) -> Result<(), TaskError> {
        match event {
            SessionEvent::Task(signature, event) => {
                if signature.id != state.signature.id {
                    Logger::warning(format!(
                        "task {} received event for task {}",
                        state.signature.id.0, signature.id.0
                    ));
                    return Ok(());
                }
                Self::dispatch_task(state, event)
            }
            SessionEvent::TaskUpdate(status) => {
                let error = Self::terminal_task_error(&status);
                Self::send_session_status(state, status);
                if let Some(error) = error {
                    Err(error)
                } else {
                    Ok(())
                }
            }
            SessionEvent::TaskCreate(_) => {
                Logger::warning(format!(
                    "task {} received unsupported TaskCreate event",
                    state.signature.id.0
                ));
                Ok(())
            }
            SessionEvent::Executor(_) => {
                Logger::warning(format!(
                    "task {} received unsupported Executor event",
                    state.signature.id.0
                ));
                Ok(())
            }
        }
    }

    fn dispatch_task(state: &Arc<TaskState>, event: TaskEvent) -> Result<(), TaskError> {
        match event {
            TaskEvent::Plan(signature, event) => {
                Self::dispatch_plan(state, signature, event);
                Ok(())
            }
            TaskEvent::PlanCreate(draft) => {
                Self::create_plan(state, draft);
                Ok(())
            }
            TaskEvent::PlanUpdate(status) => {
                let failed = matches!(status, PlanStatus::Fail);
                Self::on_plan_update(state, status);
                if failed {
                    Err(TaskError::PlanFailed)
                } else {
                    Ok(())
                }
            }
            TaskEvent::Cancel => {
                Self::cancel_task(state);
                Err(TaskError::Canceled)
            }
        }
    }

    fn create_plan(state: &Arc<TaskState>, draft: PlanDraft) {
        let signature = PlanSignature::new(state.signature.clone(), "plan".to_owned());
        let plan = match Plan::from_draft(state, signature.clone(), draft) {
            Ok(plan) => plan,
            Err(error) => {
                let reason = format!("discarding invalid plan draft: {error:?}");
                Logger::warning(format!("{reason} (task {})", state.signature.id.0));
                Self::send_session_status(state, TaskStatus::Failed { reason });
                return;
            }
        };
        Logger::debug(format!(
            "running plan {} with {} step(s) (task {})",
            signature.id.0,
            plan.run_steps.len(),
            state.signature.id.0
        ));
        let step_signatures = plan.run_step_signatures();
        if let Err(error) = state
            .plan_hub
            .insert(signature.clone(), plan.clone(), step_signatures)
        {
            let reason = format!("failed to insert task plan: {error:?}");
            Logger::error(format!("{reason} (task {})", state.signature.id.0));
            Self::send_session_status(state, TaskStatus::Failed { reason });
            return;
        }
        plan.start_run_steps();
    }

    fn dispatch_plan(state: &Arc<TaskState>, signature: PlanSignature, event: PlanEvent) {
        let event_name = format!("{event:?}");
        match state.plan_hub.get(&signature) {
            Ok(plan) => {
                if plan.sender().send(event).is_err() {
                    Logger::warning(format!(
                        "plan {} event {event_name} dispatch failed: worker stopped (task {})",
                        signature.id.0, signature.task.id.0
                    ));
                }
            }
            Err(_) => {
                Logger::error(format!(
                    "plan {} event {event_name} not dispatched: plan not found (task {})",
                    signature.id.0, signature.task.id.0
                ));
            }
        }
    }

    fn on_plan_update(state: &Arc<TaskState>, status: PlanStatus) {
        match status {
            PlanStatus::Success => {
                Logger::debug(format!("task {} plan completed", state.signature.id.0));
            }
            PlanStatus::Fail => {
                Self::send_session_status(
                    state,
                    TaskStatus::Failed {
                        reason: "plan failed".to_owned(),
                    },
                );
            }
        }
    }

    fn cancel_task(state: &Arc<TaskState>) {
        Logger::log(format!("task {} canceled", state.signature.id.0));
        for signature in state.plan_hub.list().unwrap_or_default() {
            Self::dispatch_plan(state, signature, PlanEvent::Cancel);
        }
        Self::send_session_status(state, TaskStatus::Canceled);
    }

    fn send_session_status(state: &TaskState, status: TaskStatus) {
        if state
            .session_tx
            .send(SessionEvent::TaskUpdate(status))
            .is_err()
        {
            Logger::warning(format!(
                "task {} status update failed: session worker stopped",
                state.signature.id.0
            ));
        }
    }

    fn terminal_task_error(status: &TaskStatus) -> Option<TaskError> {
        match status {
            TaskStatus::Canceled => Some(TaskError::Canceled),
            TaskStatus::Succeed(_) => Some(TaskError::Succeeded),
            TaskStatus::Failed { .. } => Some(TaskError::Failed),
            TaskStatus::Created | TaskStatus::Started => None,
        }
    }
}
