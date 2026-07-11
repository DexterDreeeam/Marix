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
        let payload = serde_json::json!({
            "model": self.config.model.trim(),
            "messages": [
                {
                    "role": "user",
                    "content": request.prompt
                }
            ],
            "stream": true
        });
        let mut response = self
            .client
            .post(self.config.endpoint.trim())
            .bearer_auth(self.config.api_key.trim())
            .json(&payload)
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
        let payload = serde_json::json!({
            "model": self.config.model.trim(),
            "messages": [
                {
                    "role": "user",
                    "content": request.prompt
                }
            ],
            "stream": true
        });
        let config = self.config.clone();
        let client = self.async_client.clone();
        let (sender, receiver) = build_async_channel();
        tokio::spawn(async move {
            if let Err(error) = Self::request_async_stream(client, config, payload, sender).await {
                Logger::error(format!("deepseek async stream response failed: {error}",));
            }
        });

        Ok(receiver)
    }
}

impl DeepseekBackend {
    async fn request_async_stream(
        client: reqwest::Client,
        config: DeepseekConfig,
        payload: serde_json::Value,
        sender: AsyncSender<ModelResponse>,
    ) -> Result<(), ModelBackendError> {
        let mut response = client
            .post(config.endpoint.trim())
            .bearer_auth(config.api_key.trim())
            .json(&payload)
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
        let mut pending = String::new();
        let mut seq_count = 0;
        let mut buffer = [0_u8; 8192];

        loop {
            while let Some(event) = Self::take_next_sse_event(&mut pending) {
                if Self::handle_sse_event(&event, sender, &mut seq_count)? {
                    return Ok(());
                }
            }
            let bytes_read = response.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            pending.push_str(&String::from_utf8_lossy(&buffer[..bytes_read]));
        }

        if !pending.trim().is_empty() {
            if Self::handle_sse_event(&pending, sender, &mut seq_count)? {
                return Ok(());
            }
        }

        let _ = sender.send(ModelResponse {
            content: String::new(),
            seq: seq_count,
            complete: true,
        });

        Ok(())
    }

    async fn stream_async_response(
        response: &mut reqwest::Response,
        sender: &AsyncSender<ModelResponse>,
    ) -> Result<(), ModelBackendError> {
        let mut pending = String::new();
        let mut seq_count = 0;

        loop {
            while let Some(event) = Self::take_next_sse_event(&mut pending) {
                if Self::handle_sse_event(&event, sender, &mut seq_count)? {
                    return Ok(());
                }
            }
            let Some(chunk) = response.chunk().await? else {
                break;
            };
            pending.push_str(&String::from_utf8_lossy(chunk.as_ref()));
        }

        if !pending.trim().is_empty() {
            if Self::handle_sse_event(&pending, sender, &mut seq_count)? {
                return Ok(());
            }
        }

        let _ = sender.send_response(ModelResponse {
            content: String::new(),
            seq: seq_count,
            complete: true,
        });

        Ok(())
    }

    fn take_next_sse_event(pending: &mut String) -> Option<String> {
        let line_feed = pending.find("\n\n").map(|index| (index, 2));
        let carriage_return = pending.find("\r\n\r\n").map(|index| (index, 4));
        let (index, separator_length) = match (line_feed, carriage_return) {
            (Some(left), Some(right)) => {
                if left.0 <= right.0 {
                    left
                } else {
                    right
                }
            }
            (Some(boundary), None) | (None, Some(boundary)) => boundary,
            (None, None) => return None,
        };
        let event = pending[..index].to_owned();
        pending.drain(..index + separator_length);
        Some(event)
    }

    fn handle_sse_event(
        event: &str,
        sender: &impl ModelResponseSender,
        seq_count: &mut usize,
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
                let _ = sender.send_response(ModelResponse {
                    content: String::new(),
                    seq: *seq_count,
                    complete: true,
                });
                return Ok(true);
            }
            let value: serde_json::Value = serde_json::from_str(data)?;
            let content = value
                .get("choices")
                .and_then(serde_json::Value::as_array)
                .and_then(|choices| choices.first())
                .and_then(|choice| choice.get("delta"))
                .and_then(|delta| delta.get("content"))
                .and_then(serde_json::Value::as_str)
                .filter(|content| !content.is_empty());
            if let Some(content) = content {
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
        }

        Ok(false)
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
