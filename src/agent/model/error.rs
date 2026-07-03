use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelBackendError {
    Unavailable(String),
    RequestFailed(String),
    InvalidResponse(String),
}

// -- Private -- //

impl fmt::Display for ModelBackendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        panic!("not implemented")
    }
}

impl std::error::Error for ModelBackendError {}
