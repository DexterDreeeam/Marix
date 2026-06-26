#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SessionStatus {
    Created,
    Running,
    Stopping,
    Stopped,
    Failed,
}

pub(crate) struct SessionContext;

impl SessionContext {
    pub(crate) fn new() -> Self {
        panic!("not implemented")
    }

    pub(crate) fn status(&self) -> SessionStatus {
        panic!("not implemented")
    }
}
