use crate::agent::engine::{SessionBrief, TaskStep};
use crate::common::channel::SessionTaskId;
use crate::common::message::RequestMessageEnvelope;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ModelContext {
    pub(crate) session_brief: SessionBrief,
    pub(crate) task_id: SessionTaskId,
    pub(crate) request: RequestMessageEnvelope,
    pub(crate) recent_steps: Vec<TaskStep>,
}
