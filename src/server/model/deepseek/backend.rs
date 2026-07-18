#[path = "request.rs"]
mod request;
#[path = "stream.rs"]
mod stream;

use std::fmt;

use marix_common::external::*;
use marix_common::{AsyncSender, Config, DeepseekConfig, Logger};
use marix_protocol::ToolPreview;

use crate::model::{ModelBackendError, ModelRequest, ModelResponse};

#[derive(Clone)]
pub struct DeepseekBackend {
    config: DeepseekConfig,
    async_client: reqwest::Client,
}

impl DeepseekBackend {
    pub fn new() -> Self {
        let config = Config::load()
            .unwrap_or_else(|error| panic!("failed to load config: {error}"))
            .model
            .deepseek;
        Self {
            config,
            async_client: reqwest::Client::new(),
        }
    }
}

// -- Private -- //

impl DeepseekBackend {
    fn build_payload(&self, request: &ModelRequest) -> Result<String, ModelBackendError> {
        let mut messages = Vec::with_capacity(request.prompts.len() + 1);
        messages.push(serde_json::json!({
            "role": "system",
            "content": &request.system
        }));
        messages.extend(request.prompts.iter().map(|prompt| {
            serde_json::json!({
                "role": "user",
                "content": prompt
            })
        }));
        let mut payload = serde_json::json!({
            "model": self.config.model.trim(),
            "messages": messages,
            "stream": true
        });
        match request.tools.as_ref() {
            None => {
                payload["response_format"] = serde_json::json!({
                    "type": "json_object"
                });
            }
            Some(tools) => {
                payload["tools"] = serde_json::json!(self.build_tools(tools)?);
                payload["tool_choice"] = serde_json::json!("required");
            }
        }
        serde_json::to_string(&payload).map_err(|error| {
            ModelBackendError::RequestFailed(format!(
                "failed to serialize Deepseek request payload: {error}",
            ))
        })
    }

    async fn request_stream_response(
        client: reqwest::Client,
        config: DeepseekConfig,
        raw: String,
        native_tools: bool,
        sender: AsyncSender<ModelResponse>,
    ) -> Result<(), ModelBackendError> {
        let mut response = client
            .post(config.endpoint.trim())
            .bearer_auth(config.api_key.trim())
            .header("content-type", "application/json")
            .body(raw)
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await?;
            Self::log_response(&body);
            Logger::error(format!("deepseek stream request failed: {status}"));
            return Err(ModelBackendError::RequestFailed(format!(
                "Deepseek request failed with {status}: {body}"
            )));
        }

        Logger::debug("deepseek stream established");
        Self::stream_response(&mut response, &sender, native_tools).await
    }

    fn build_tools(
        &self,
        tools: &[ToolPreview],
    ) -> Result<Vec<serde_json::Value>, ModelBackendError> {
        tools
            .iter()
            .map(|tool| {
                let parameters: serde_json::Value =
                    serde_json::from_str(&tool.input).map_err(|error| {
                        ModelBackendError::RequestFailed(format!(
                            "tool `{}` input schema is invalid JSON: {error}",
                            tool.name,
                        ))
                    })?;
                Ok(serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": &tool.name,
                        "description": &tool.description,
                        "parameters": parameters
                    }
                }))
            })
            .collect()
    }

    fn log_response(content: &str) {
        Logger::log(format!("[Model Relay][Response] {content}"));
    }
}

impl fmt::Debug for DeepseekBackend {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DeepseekBackend")
            .field("api_key_configured", &(!self.config.api_key.is_empty()))
            .field("endpoint", &self.config.endpoint)
            .field("model", &self.config.model)
            .finish()
    }
}
