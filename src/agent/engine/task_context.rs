use std::sync::{mpsc, Arc, Mutex};

use crate::agent::frontdoor::Task;
use crate::common::channel::SessionTaskId;
use crate::common::message::RequestMessageEnvelope;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TaskRuntimeEvent {
    Status(TaskStatus),
    ModelRequest(String),
    ModelResponse(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TaskStatus {
    Created,
    Running,
    Stopped,
    Succeeded,
    Failed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TaskStepKind {
    Model,
    Tool,
    UserDecision,
    UserAuthorization,
}

/// One executed step in a task. Steps are the bottom-level execution unit that
/// the agent runs; the plan/job grouping that produced them lives in Plan/Job,
/// so a step stays a flat execution record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaskStep {
    pub(crate) sequence: usize,
    pub(crate) kind: TaskStepKind,
    pub(crate) output: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaskResult {
    pub(crate) status: TaskStatus,
    pub(crate) output: Option<String>,
    pub(crate) error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaskBrief {
    pub(crate) task_id: SessionTaskId,
    pub(crate) request: String,
    pub(crate) result: TaskResult,
    pub(crate) content: String,
    pub(crate) step_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaskTrace {
    pub(crate) steps: Vec<TaskStep>,
    pub(crate) result: Option<TaskResult>,
    pub(crate) brief: Option<TaskBrief>,
}

#[derive(Clone)]
pub(crate) struct TaskContext {
    pub(super) task_id: SessionTaskId,
    pub(super) initial_message: Arc<RequestMessageEnvelope>,
    pub(super) task: Arc<Mutex<Task>>,
    pub(super) status: Arc<Mutex<TaskStatus>>,
    pub(super) runtime_tx: Arc<Mutex<Option<mpsc::Sender<TaskRuntimeEvent>>>>,
}

impl TaskContext {
    pub(crate) fn task_id(&self) -> SessionTaskId {
        self.task_id
    }

    pub(crate) fn initial_message(&self) -> &RequestMessageEnvelope {
        self.initial_message.as_ref()
    }

    pub(crate) fn status(&self) -> TaskStatus {
        self.status
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    pub(crate) fn trace(&self) -> &TaskTrace {
        panic!("not implemented")
    }
}
