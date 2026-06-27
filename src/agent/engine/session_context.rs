#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SessionStatus {
    Created,
    Running,
    Stopping,
    Stopped,
    Failed,
}

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
}
