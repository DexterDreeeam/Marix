use std::fmt;
use std::io::Read;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

use reqwest::blocking::{Client, Response};
use serde_json::{json, Value};

use crate::common::config::DeepseekConfig;

use super::backend::ModelBackendImpl;
use super::{ModelBackendError, ModelRequest, ModelResponse};

#[derive(Clone)]
pub struct DeepseekBackend<'config> {
    config: &'config DeepseekConfig,
    client: Client,
}

impl<'config> DeepseekBackend<'config> {
    pub fn new(config: &'config DeepseekConfig) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }
}

impl ModelBackendImpl for DeepseekBackend<'_> {
    fn ready(&self) -> Result<(), ModelBackendError> {
        if self.config.api_key.trim().is_empty() {
            return Err(ModelBackendError::Unavailable(
                "Deepseek API key is not configured".to_owned(),
            ));
        }
        if self.config.endpoint.trim().is_empty() {
            return Err(ModelBackendError::Unavailable(
                "Deepseek endpoint is not configured".to_owned(),
            ));
        }
        if self.config.model.trim().is_empty() {
            return Err(ModelBackendError::Unavailable(
                "Deepseek model is not configured".to_owned(),
            ));
        }
        Ok(())
    }

    fn send(
        &mut self,
        request: ModelRequest,
    ) -> Result<Receiver<ModelResponse>, ModelBackendError> {
        let payload = json!({
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

        let (sender, receiver) = channel();
        thread::spawn(move || {
            if let Err(error) = stream_response(&mut response, &sender) {
                let _ = sender.send(ModelResponse::Failed(error));
            }
        });

        Ok(receiver)
    }
}

fn stream_response(
    response: &mut Response,
    sender: &Sender<ModelResponse>,
) -> Result<(), ModelBackendError> {
    let mut pending = String::new();
    let mut buffer = [0_u8; 8192];

    loop {
        while let Some(event) = take_next_sse_event(&mut pending) {
            if handle_sse_event(&event, sender)? {
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
        handle_sse_event(&pending, sender)?;
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
        let value: Value = serde_json::from_str(data)?;
        let content = value
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
            .and_then(|choice| choice.get("delta"))
            .and_then(|delta| delta.get("content"))
            .and_then(Value::as_str)
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

impl fmt::Debug for DeepseekBackend<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DeepseekBackend")
            .field("api_key_configured", &(!self.config.api_key.is_empty()))
            .field("endpoint", &self.config.endpoint)
            .field("model", &self.config.model)
            .finish()
    }
}
