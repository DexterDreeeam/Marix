use std::fmt;
use std::io::Read;
use std::thread;

use marix_common::external::*;
use marix_common::{Config, DeepseekConfig, Receiver, Sender, build_channel};

use super::backend::ModelBackendImpl;
use super::{ModelBackendError, ModelRequest, ModelResponse};

#[derive(Clone)]
pub struct DeepseekBackend {
    config: DeepseekConfig,
    client: reqwest::blocking::Client,
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
        }
    }
}

// -- Private -- //

impl ModelBackendImpl for DeepseekBackend {
    fn request(
        &mut self,
        request: ModelRequest,
    ) -> Result<Receiver<ModelResponse>, ModelBackendError> {
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
            return Err(ModelBackendError::RequestFailed(format!(
                "Deepseek request failed with {status}: {body}"
            )));
        }

        let (sender, receiver) = build_channel();
        thread::spawn(move || {
            if let Err(error) = Self::stream_response(&mut response, &sender) {
                let _ = sender.send(ModelResponse::Failed(error));
            }
        });

        Ok(receiver)
    }
}

impl DeepseekBackend {
    fn stream_response(
        response: &mut reqwest::blocking::Response,
        sender: &Sender<ModelResponse>,
    ) -> Result<(), ModelBackendError> {
        let mut pending = String::new();
        let mut buffer = [0_u8; 8192];

        loop {
            while let Some(event) = Self::take_next_sse_event(&mut pending) {
                if Self::handle_sse_event(&event, sender)? {
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
            Self::handle_sse_event(&pending, sender)?;
        }

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
        sender: &Sender<ModelResponse>,
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
                if sender
                    .send(ModelResponse::Content(content.to_owned()))
                    .is_err()
                {
                    return Ok(true);
                }
            }
        }

        Ok(false)
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
