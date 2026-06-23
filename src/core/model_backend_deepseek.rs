use std::process::Command;

use serde_json::{json, Value};

use crate::model_backend::{ModelBackend, ModelBackendError, ModelBackendOutput};
use crate::preprocess::PreprocessOutput;

const DEFAULT_DEEPSEEK_API_ENDPOINT: &str = "https://api.deepseek.com/chat/completions";
const DEFAULT_DEEPSEEK_MODEL: &str = "deepseek-chat";

#[derive(Debug, Clone)]
pub struct ModelBackendDeepseek {
    api_key: Option<String>,
    endpoint: String,
    model: String,
}

impl ModelBackendDeepseek {
    pub fn from_env() -> Self {
        Self {
            api_key: std::env::var("DEEPSEEK_API_KEY").ok(),
            endpoint: std::env::var("DEEPSEEK_API_ENDPOINT")
                .unwrap_or_else(|_| DEFAULT_DEEPSEEK_API_ENDPOINT.to_owned()),
            model: std::env::var("DEEPSEEK_MODEL")
                .unwrap_or_else(|_| DEFAULT_DEEPSEEK_MODEL.to_owned()),
        }
    }

    fn request_response(
        &self,
        input: PreprocessOutput,
    ) -> Result<ModelBackendOutput, ModelBackendError> {
        let Some(api_key) = self.api_key.as_deref().filter(|value| !value.is_empty()) else {
            return Err(ModelBackendError::Unavailable(
                "DEEPSEEK_API_KEY is not configured".to_owned(),
            ));
        };
        let payload = json!({
            "model": self.model,
            "messages": [
                {
                    "role": "user",
                    "content": input.prompt
                }
            ],
            "stream": false
        });
        let output = Command::new("curl")
            .arg("--silent")
            .arg("--show-error")
            .arg("--fail-with-body")
            .arg("-X")
            .arg("POST")
            .arg(&self.endpoint)
            .arg("-H")
            .arg("Content-Type: application/json")
            .arg("-H")
            .arg(format!("Authorization: Bearer {api_key}"))
            .arg("-d")
            .arg(payload.to_string())
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            let detail = if stderr.is_empty() { stdout } else { stderr };
            return Err(ModelBackendError::RequestFailed(detail));
        }

        let response: Value = serde_json::from_slice(&output.stdout)?;
        let content = response
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("content"))
            .and_then(Value::as_str)
            .filter(|content| !content.trim().is_empty())
            .ok_or_else(|| {
                ModelBackendError::InvalidResponse("missing choices[0].message.content".to_owned())
            })?;

        Ok(ModelBackendOutput::new(content))
    }
}

impl ModelBackend for ModelBackendDeepseek {
    fn wait_response(
        &mut self,
        input: PreprocessOutput,
    ) -> Result<ModelBackendOutput, ModelBackendError> {
        self.request_response(input)
    }
}
