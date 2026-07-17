use std::fmt;
use std::io::Read;
use std::thread;

use marix_common::external::*;
use marix_common::{
    AsyncSender, Config, DeepseekConfig, Logger, Receiver, Sender, build_async_channel,
    build_channel,
};

use super::backend::ModelBackendImpl;
use super::{ModelBackendError, ModelRequest, ModelResponse, ModelResponseAsyncReceiver};

#[derive(Clone)]
pub struct DeepseekBackend {
    config: DeepseekConfig,
    client: reqwest::blocking::Client,
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
            client: reqwest::blocking::Client::new(),
            async_client: reqwest::Client::new(),
        }
    }
}

// -- Private -- //

trait ModelResponseSender {
    fn send_response(&self, response: ModelResponse) -> bool;
}

impl ModelBackendImpl for DeepseekBackend {
    fn request(
        &mut self,
        request: ModelRequest,
    ) -> Result<Receiver<ModelResponse>, ModelBackendError> {
        Logger::debug(format!(
            "deepseek request: model '{}'",
            self.config.model.trim()
        ));
        let raw = self.build_payload(&request)?;
        Logger::log(format!("[Model Relay][Request] {raw}"));
        let mut response = self
            .client
            .post(self.config.endpoint.trim())
            .bearer_auth(self.config.api_key.trim())
            .header("content-type", "application/json")
            .body(raw)
            .send()?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text()?;
            Logger::error(format!("deepseek request failed: {status}"));
            return Err(ModelBackendError::RequestFailed(format!(
                "Deepseek request failed with {status}: {body}"
            )));
        }

        let (sender, receiver) = build_channel();
        Logger::debug("deepseek stream established");
        thread::spawn(move || {
            if let Err(error) = Self::stream_response(&mut response, &sender) {
                Logger::error(format!("deepseek stream response failed: {error}",));
            }
        });

        Ok(receiver)
    }

    fn request_async(
        &mut self,
        request: ModelRequest,
    ) -> Result<ModelResponseAsyncReceiver, ModelBackendError> {
        Logger::debug(format!(
            "deepseek async request: model '{}'",
            self.config.model.trim()
        ));
        let raw = self.build_payload(&request)?;
        Logger::log(format!("[Model Relay][Request] {raw}"));
        let config = self.config.clone();
        let client = self.async_client.clone();
        let (sender, receiver) = build_async_channel();
        tokio::spawn(async move {
            if let Err(error) = Self::request_async_stream(client, config, raw, sender).await {
                Logger::error(format!("deepseek async stream response failed: {error}",));
            }
        });

        Ok(receiver)
    }
}

impl DeepseekBackend {
    fn build_payload(&self, request: &ModelRequest) -> Result<String, ModelBackendError> {
        let tools = request
            .tools
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
            .collect::<Result<Vec<_>, ModelBackendError>>()?;
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
            "tools": tools,
            "response_format": {
                "type": "json_object"
            },
            "stream": true
        });
        if !request.tools.is_empty() {
            payload["tool_choice"] = serde_json::json!("none");
        }
        serde_json::to_string(&payload).map_err(|error| {
            ModelBackendError::RequestFailed(format!(
                "failed to serialize Deepseek request payload: {error}",
            ))
        })
    }

    async fn request_async_stream(
        client: reqwest::Client,
        config: DeepseekConfig,
        raw: String,
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
            Logger::error(format!("deepseek async request failed: {status}"));
            return Err(ModelBackendError::RequestFailed(format!(
                "Deepseek request failed with {status}: {body}"
            )));
        }

        Logger::debug("deepseek async stream established");
        Self::stream_async_response(&mut response, &sender).await
    }

    fn stream_response(
        response: &mut reqwest::blocking::Response,
        sender: &Sender<ModelResponse>,
    ) -> Result<(), ModelBackendError> {
        let mut pending = Vec::new();
        let mut accumulated = String::new();
        let mut seq_count = 0;
        let mut buffer = [0_u8; 8192];

        loop {
            while let Some(event) = Self::take_next_sse_event(&mut pending)? {
                if Self::handle_sse_event(&event, sender, &mut seq_count, &mut accumulated)? {
                    return Ok(());
                }
            }
            let bytes_read = response.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            pending.extend_from_slice(&buffer[..bytes_read]);
        }

        if !pending.iter().all(|byte| byte.is_ascii_whitespace()) {
            let event = Self::decode_sse_event(pending)?;
            if Self::handle_sse_event(&event, sender, &mut seq_count, &mut accumulated)? {
                return Ok(());
            }
        }

        Self::complete_response(sender, seq_count, &accumulated);

        Ok(())
    }

    async fn stream_async_response(
        response: &mut reqwest::Response,
        sender: &AsyncSender<ModelResponse>,
    ) -> Result<(), ModelBackendError> {
        let mut pending = Vec::new();
        let mut accumulated = String::new();
        let mut seq_count = 0;

        loop {
            while let Some(event) = Self::take_next_sse_event(&mut pending)? {
                if Self::handle_sse_event(&event, sender, &mut seq_count, &mut accumulated)? {
                    return Ok(());
                }
            }
            let Some(chunk) = response.chunk().await? else {
                break;
            };
            pending.extend_from_slice(chunk.as_ref());
        }

        if !pending.iter().all(|byte| byte.is_ascii_whitespace()) {
            let event = Self::decode_sse_event(pending)?;
            if Self::handle_sse_event(&event, sender, &mut seq_count, &mut accumulated)? {
                return Ok(());
            }
        }

        Self::complete_response(sender, seq_count, &accumulated);

        Ok(())
    }

    fn take_next_sse_event(pending: &mut Vec<u8>) -> Result<Option<String>, ModelBackendError> {
        let line_feed = pending
            .windows(2)
            .position(|window| window == b"\n\n")
            .map(|index| (index, 2));
        let carriage_return = pending
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .map(|index| (index, 4));
        let (index, separator_length) = match (line_feed, carriage_return) {
            (Some(left), Some(right)) => {
                if left.0 <= right.0 {
                    left
                } else {
                    right
                }
            }
            (Some(boundary), None) | (None, Some(boundary)) => boundary,
            (None, None) => return Ok(None),
        };
        let mut event = pending
            .drain(..index + separator_length)
            .collect::<Vec<_>>();
        event.truncate(index);
        Self::decode_sse_event(event).map(Some)
    }

    fn decode_sse_event(event: Vec<u8>) -> Result<String, ModelBackendError> {
        String::from_utf8(event).map_err(|error| {
            ModelBackendError::InvalidResponse(format!(
                "Deepseek SSE event is not valid UTF-8: {error}",
            ))
        })
    }

    fn handle_sse_event(
        event: &str,
        sender: &impl ModelResponseSender,
        seq_count: &mut usize,
        accumulated: &mut String,
    ) -> Result<bool, ModelBackendError> {
        for line in event.lines() {
            let Some(data) = line.trim().strip_prefix("data:") else {
                continue;
            };
            let data = data.trim();
            if data.is_empty() {
                continue;
            }
            if data == "[DONE]" {
                Self::complete_response(sender, *seq_count, accumulated);
                return Ok(true);
            }
            let value: serde_json::Value = serde_json::from_str(data)?;
            let choice = value
                .get("choices")
                .and_then(serde_json::Value::as_array)
                .and_then(|choices| choices.first());
            let delta = choice.and_then(|choice| choice.get("delta"));
            if delta
                .and_then(|delta| delta.get("tool_calls"))
                .is_some_and(|tool_calls| {
                    !tool_calls.is_null()
                        && !tool_calls
                            .as_array()
                            .is_some_and(|tool_calls| tool_calls.is_empty())
                })
            {
                Self::log_response(accumulated);
                return Err(ModelBackendError::InvalidResponse(
                    "Deepseek returned native tool_calls; \
                     custom decision JSON content is required"
                        .to_owned(),
                ));
            }
            let content = delta
                .and_then(|delta| delta.get("content"))
                .and_then(serde_json::Value::as_str)
                .filter(|content| !content.is_empty());
            if let Some(content) = content {
                accumulated.push_str(content);
                let response = ModelResponse {
                    content: content.to_owned(),
                    seq: *seq_count,
                    complete: false,
                };
                if !sender.send_response(response) {
                    return Ok(true);
                }
                *seq_count += 1;
            }
            let complete = choice
                .and_then(|choice| choice.get("finish_reason"))
                .is_some_and(|reason| !reason.is_null());
            if complete {
                Self::complete_response(sender, *seq_count, accumulated);
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn complete_response(sender: &impl ModelResponseSender, seq_count: usize, content: &str) {
        let _ = sender.send_response(ModelResponse {
            content: String::new(),
            seq: seq_count,
            complete: true,
        });
        Self::log_response(content);
    }

    fn log_response(content: &str) {
        Logger::log(format!("[Model Relay][Response] {content}"));
    }
}

impl ModelResponseSender for Sender<ModelResponse> {
    fn send_response(&self, response: ModelResponse) -> bool {
        self.send(response).is_ok()
    }
}

impl ModelResponseSender for AsyncSender<ModelResponse> {
    fn send_response(&self, response: ModelResponse) -> bool {
        self.send(response).is_ok()
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
