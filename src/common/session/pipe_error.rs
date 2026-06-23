#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PipeError {
    Unavailable(String),
    ConnectionClosed,
    SendFailed(String),
    ReceiveFailed(String),
}

impl std::fmt::Display for PipeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable(reason) => write!(formatter, "pipe unavailable: {reason}"),
            Self::ConnectionClosed => write!(formatter, "pipe connection closed"),
            Self::SendFailed(reason) => write!(formatter, "pipe send failed: {reason}"),
            Self::ReceiveFailed(reason) => write!(formatter, "pipe receive failed: {reason}"),
        }
    }
}

impl std::error::Error for PipeError {}
