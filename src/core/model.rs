use std::process::Command;

use serde_json::{json, Value};

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
    RequestFailed(String),
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable(reason) => write!(formatter, "model backend unavailable: {reason}"),
            Self::RequestFailed(reason) => write!(formatter, "model request failed: {reason}"),
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

#[derive(Debug, Clone, Copy, Default)]
pub struct EchoModelBackend;

impl ModelBackend for EchoModelBackend {
    fn generate(&self, request: ModelRequest) -> Result<ModelResponse, ModelError> {
        Ok(ModelResponse {
            content: request.prompt,
        })
    }
}

#[derive(Debug, Clone)]
pub struct DeepSeekModelBackend {
    endpoint: String,
    model: String,
    api_key: String,
}

impl DeepSeekModelBackend {
    pub fn from_config(config: &Value) -> Result<Self, ModelError> {
        let model_id = config
            .get("core")
            .and_then(|core| core.get("model"))
            .and_then(Value::as_str)
            .unwrap_or("deepseek-default");
        let deployment_model = find_deployment_model(config, model_id);
        let base_url = std::env::var("DEEPSEEK_BASE_URL")
            .ok()
            .or_else(|| {
                deployment_model
                    .and_then(|model| model.get("baseUrl"))
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
            })
            .unwrap_or_else(|| "https://api.deepseek.com".to_owned());
        let path = std::env::var("DEEPSEEK_CHAT_COMPLETIONS_PATH")
            .ok()
            .or_else(|| {
                deployment_model
                    .and_then(|model| model.get("chatCompletionsPath"))
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
            })
            .unwrap_or_else(|| "/chat/completions".to_owned());
        let api_key = std::env::var("DEEPSEEK_API_KEY")
            .ok()
            .or_else(|| {
                deployment_model
                    .and_then(|model| model.get("apiKey"))
                    .and_then(Value::as_str)
                    .filter(|value| !value.starts_with('{'))
                    .map(ToOwned::to_owned)
            })
            .ok_or_else(|| {
                ModelError::Unavailable("DEEPSEEK_API_KEY is not configured".to_owned())
            })?;
        let model = std::env::var("DEEPSEEK_MODEL").unwrap_or_else(|_| "deepseek-chat".to_owned());

        Ok(Self {
            endpoint: format!("{}{}", base_url.trim_end_matches('/'), path),
            model,
            api_key,
        })
    }
}

impl ModelBackend for DeepSeekModelBackend {
    fn generate(&self, request: ModelRequest) -> Result<ModelResponse, ModelError> {
        let payload = json!({
            "model": self.model,
            "messages": [
                {
                    "role": "user",
                    "content": request.prompt
                }
            ],
            "temperature": 0.2
        });
        let output = Command::new("curl")
            .arg("-sS")
            .arg("-X")
            .arg("POST")
            .arg(&self.endpoint)
            .arg("-H")
            .arg("Content-Type: application/json")
            .arg("-H")
            .arg(format!("Authorization: Bearer {}", self.api_key))
            .arg("-d")
            .arg(payload.to_string())
            .output()
            .map_err(|error| ModelError::Unavailable(format!("curl unavailable: {error}")))?;

        if !output.status.success() {
            return Err(ModelError::RequestFailed(
                String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            ));
        }

        let body: Value = serde_json::from_slice(&output.stdout).map_err(|error| {
            ModelError::RequestFailed(format!("invalid JSON response: {error}"))
        })?;
        let content = body
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("content"))
            .and_then(Value::as_str)
            .ok_or_else(|| {
                ModelError::RequestFailed("missing assistant message content".to_owned())
            })?;

        Ok(ModelResponse {
            content: content.to_owned(),
        })
    }
}

fn find_deployment_model<'a>(config: &'a Value, model_id: &str) -> Option<&'a Value> {
    config
        .get("deployment")
        .and_then(|deployment| deployment.get("models"))
        .and_then(Value::as_array)
        .and_then(|models| {
            models
                .iter()
                .find(|model| model.get("id").and_then(Value::as_str) == Some(model_id))
        })
}
