use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoggingError {
    Config(String),
    Io(String),
    Clock(String),
}

// -- Private -- //

impl From<std::io::Error> for LoggingError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

impl From<std::time::SystemTimeError> for LoggingError {
    fn from(error: std::time::SystemTimeError) -> Self {
        Self::Clock(error.to_string())
    }
}

impl fmt::Display for LoggingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(message) => write!(formatter, "logging config error: {message}"),
            Self::Io(message) => write!(formatter, "logging I/O error: {message}"),
            Self::Clock(message) => write!(formatter, "logging clock error: {message}"),
        }
    }
}

impl std::error::Error for LoggingError {}
