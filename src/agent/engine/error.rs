#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoopEngineError {
    SessionClosed,
    TaskClosed,
    BackendUnavailable(String),
    BackendFailed(String),
}
