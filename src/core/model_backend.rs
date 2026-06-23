use crate::preprocess::PreprocessOutput;

pub trait ModelBackend: std::fmt::Debug {
    fn wait_response(
        &mut self,
        input: PreprocessOutput,
    ) -> Result<ModelBackendOutput, ModelBackendError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelBackendOutput {
    pub content: String,
}

impl ModelBackendOutput {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
        }
    }
}

#[derive(Debug)]
pub enum ModelBackendError {
    Unavailable(String),
    RequestFailed(String),
    InvalidResponse(String),
}

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

impl std::fmt::Display for ModelBackendError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
