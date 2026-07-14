use std::fmt;

#[derive(Debug)]
pub(crate) enum HttpError {
    Runtime(String),
    Bind(String),
    Serve(String),
}

// -- Private -- //

impl fmt::Display for HttpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Runtime(message) => write!(formatter, "HTTP runtime error: {message}"),
            Self::Bind(message) => write!(formatter, "HTTP bind error: {message}"),
            Self::Serve(message) => write!(formatter, "HTTP serve error: {message}"),
        }
    }
}

impl std::error::Error for HttpError {}
