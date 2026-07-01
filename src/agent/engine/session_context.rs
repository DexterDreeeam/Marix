use super::task_context::TaskBrief;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SessionStatus {
    Created,
    Running,
    Stopping,
    Stopped,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionBrief {
    pub(crate) revision: u64,
    pub(crate) rolling_summary: Option<String>,
    pub(crate) recent_tasks: Vec<TaskBrief>,
}

#[derive(Clone)]
pub(crate) struct SessionContext {
    status: SessionStatus,
}

impl SessionContext {
    pub(crate) fn new() -> Self {
        Self {
            status: SessionStatus::Created,
        }
    }

    pub(crate) fn status(&self) -> SessionStatus {
        self.status
    }

    pub(crate) fn brief(&self) -> &SessionBrief {
        panic!("not implemented")
    }
}
