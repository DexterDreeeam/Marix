use std::fmt;

use crate::common::external::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelBackendError {
    Unavailable(String),
    RequestFailed(String),
    InvalidResponse(String),
}

// -- Private -- //

impl From<std::io::Error> for ModelBackendError {
    fn from(error: std::io::Error) -> Self {
        Self::RequestFailed(error.to_string())
    }
}

impl From<serde_json::Error> for ModelBackendError {
    fn from(error: serde_json::Error) -> Self {
        Self::InvalidResponse(error.to_string())
    }
}

impl From<reqwest::Error> for ModelBackendError {
    fn from(error: reqwest::Error) -> Self {
        Self::RequestFailed(error.to_string())
    }
}

impl fmt::Display for ModelBackendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(reason) => write!(formatter, "model backend unavailable: {reason}"),
            Self::RequestFailed(reason) => {
                write!(formatter, "model backend request failed: {reason}")
            }
            Self::InvalidResponse(reason) => {
                write!(formatter, "model backend response is invalid: {reason}")
            }
        }
    }
}

impl std::error::Error for ModelBackendError {}
