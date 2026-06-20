use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRequest {
    pub prompt: String,
    pub tokens: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelResponse {
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelError {
    Unavailable(String),
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable(reason) => write!(formatter, "model backend unavailable: {reason}"),
        }
    }
}

impl std::error::Error for ModelError {}

pub trait ModelBackend {
    fn generate(&self, request: ModelRequest) -> Result<ModelResponse, ModelError>;
}

pub trait RemoteModelBackend: ModelBackend {
    fn remote_config(&self) -> &Value;
}

pub trait LocalModelBackend: ModelBackend {
    fn local_config(&self) -> &Value;
}
