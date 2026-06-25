use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChannelError {
    Disconnected,
    InvalidState(String),
    ReceiveFailed(String),
    SendFailed(String),
    TransportFailed(String),
    Unsupported(String),
}

impl From<std::io::Error> for ChannelError {
    fn from(error: std::io::Error) -> Self {
        Self::TransportFailed(error.to_string())
    }
}

impl fmt::Display for ChannelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disconnected => formatter.write_str("channel disconnected"),
            Self::InvalidState(message) => write!(formatter, "invalid channel state: {message}"),
            Self::ReceiveFailed(message) => write!(formatter, "channel receive failed: {message}"),
            Self::SendFailed(message) => write!(formatter, "channel send failed: {message}"),
            Self::TransportFailed(message) => {
                write!(formatter, "channel transport failed: {message}")
            }
            Self::Unsupported(message) => {
                write!(formatter, "unsupported channel operation: {message}")
            }
        }
    }
}

impl std::error::Error for ChannelError {}
