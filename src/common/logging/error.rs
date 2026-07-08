use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoggingError {
    Config(String),
    Io(String),
    Database(String),
    Serialization(String),
    Channel(String),
    AlreadyConfigured,
    NotHosting,
}

// -- Private -- //

impl From<std::io::Error> for LoggingError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

impl fmt::Display for LoggingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(message) => write!(formatter, "telemetry config error: {message}"),
            Self::Io(message) => write!(formatter, "telemetry I/O error: {message}"),
            Self::Database(message) => write!(formatter, "telemetry database error: {message}"),
            Self::Serialization(message) => {
                write!(formatter, "telemetry serialization error: {message}")
            }
            Self::Channel(message) => write!(formatter, "telemetry channel error: {message}"),
            Self::AlreadyConfigured => write!(formatter, "telemetry logger already configured"),
            Self::NotHosting => write!(formatter, "telemetry logger is not hosting a store"),
        }
    }
}

impl std::error::Error for LoggingError {}
