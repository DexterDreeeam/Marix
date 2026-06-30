#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogTag {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogMessage {
    pub tag: LogTag,
    pub message: String,
}

impl LogMessage {
    pub fn info(message: impl Into<String>) -> Self {
        Self {
            tag: LogTag::Info,
            message: message.into(),
        }
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            tag: LogTag::Warning,
            message: message.into(),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            tag: LogTag::Error,
            message: message.into(),
        }
    }
}

impl From<String> for LogMessage {
    fn from(message: String) -> Self {
        Self::info(message)
    }
}

impl From<&str> for LogMessage {
    fn from(message: &str) -> Self {
        Self::info(message)
    }
}
