use crate::common::channel::SessionTaskId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LoopEngineError {
    SessionClosed,
    TaskNotFound(SessionTaskId),
    TaskClosed(SessionTaskId),
    BackendUnavailable(String),
    BackendFailed(String),
    TaskFailed(String),
    StatusUnavailable(SessionTaskId),
}
