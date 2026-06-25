use std::fmt;
use std::io::Read;

use marix_common::{DynamicResponse, DynamicResponseProducer};
use reqwest::blocking::Client;
use serde_json::{json, Value};

use super::{ModelBackend, ModelBackendError, ModelBackendOutput};
use crate::preprocess::PreprocessOutput;
use crate::run_log;

const DEFAULT_DEEPSEEK_API_ENDPOINT: &str = "https://api.deepseek.com/chat/completions";
const DEFAULT_DEEPSEEK_MODEL: &str = "deepseek-chat";

#[derive(Clone)]
pub struct ModelBackendDeepseek {
    api_key: Option<String>,
    endpoint: String,
    model: String,
    client: Client,
}

impl ModelBackendDeepseek {
    pub fn new() -> Self {
        let api_key = Self::read_sanitized_env("DEEPSEEK_API_KEY");
        run_log::record(format!(
            "deepseek backend initialized: api_key_configured={}",
            api_key.is_some()
        ));
        Self {
            api_key,
            endpoint: std::env::var("DEEPSEEK_API_ENDPOINT")
                .map(|value| value.trim().to_owned())
                .unwrap_or_else(|_| DEFAULT_DEEPSEEK_API_ENDPOINT.to_owned()),
            model: std::env::var("DEEPSEEK_MODEL")
                .map(|value| value.trim().to_owned())
                .unwrap_or_else(|_| DEFAULT_DEEPSEEK_MODEL.to_owned()),
            client: Client::new(),
        }
    }

    fn stream_response(
        &self,
        input: PreprocessOutput,
        producer: &DynamicResponseProducer<ModelBackendOutput>,
    ) -> Result<(), ModelBackendError> {
        let Some(api_key) = self.api_key.as_deref().filter(|value| !value.is_empty()) else {
            run_log::record("deepseek request blocked: API key is not configured");
            return Err(ModelBackendError::Unavailable(
                "DEEPSEEK_API_KEY is not configured".to_owned(),
            ));
        };
        run_log::record(format!(
            "deepseek stream request starting: endpoint={}, model={}",
            self.endpoint, self.model
        ));
        let payload = json!({
            "model": self.model,
            "messages": [
                {
                    "role": "user",
                    "content": input.prompt
                }
            ],
            "stream": true
        });
        let mut response = self
            .client
            .post(&self.endpoint)
            .bearer_auth(api_key)
            .json(&payload)
            .send()
            .map_err(|error| {
                run_log::record(format!("deepseek request send failed: {error:?}"));
                ModelBackendError::from(error)
            })?;
        let status = response.status();
        run_log::record(format!("deepseek stream response status: {status}"));

        if !status.is_success() {
            let body = response.text()?;
            run_log::record(format!(
                "deepseek request failed: status={status}, body_bytes={}",
                body.len()
            ));
            return Err(ModelBackendError::RequestFailed(format!(
                "Deepseek request failed with {status}: {body}"
            )));
        }

        let mut pending = String::new();
        let mut buffer = [0_u8; 8192];
        loop {
            while let Some(event) = take_next_sse_event(&mut pending) {
                if handle_sse_event(&event, producer)? {
                    run_log::record("deepseek stream completed with DONE event");
                    producer.complete();
                    return Ok(());
                }
            }
            let bytes_read = response.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            pending.push_str(&String::from_utf8_lossy(&buffer[..bytes_read]));
        }
        if !pending.trim().is_empty() && handle_sse_event(&pending, producer)? {
            run_log::record("deepseek stream completed with pending DONE event");
            producer.complete();
            return Ok(());
        }

        run_log::record("deepseek stream completed at EOF");
        producer.complete();
        Ok(())
    }

    fn read_sanitized_env(name: &str) -> Option<String> {
        let raw = std::env::var(name).ok()?;
        let sanitized: String = raw
            .trim()
            .chars()
            .filter(|character| character.is_ascii_graphic())
            .collect();
        if sanitized.len() != raw.len() {
            run_log::record(format!(
                "{name} contained non-printable characters and was sanitized"
            ));
        }
        Some(sanitized)
    }
}

impl fmt::Debug for ModelBackendDeepseek {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ModelBackendDeepseek")
            .field("api_key_configured", &self.api_key.is_some())
            .field("endpoint", &self.endpoint)
            .field("model", &self.model)
            .finish()
    }
}

impl ModelBackend for ModelBackendDeepseek {
    fn request_response(
        &mut self,
        input: PreprocessOutput,
    ) -> Result<DynamicResponse<ModelBackendOutput>, ModelBackendError> {
        let (response, producer) = DynamicResponse::new(ModelBackendOutput::new(""));
        let backend = self.clone();
        producer.spawn(move |producer| {
            if let Err(error) = backend.stream_response(input, producer) {
                producer.fail(error.to_string());
            }
        });
        Ok(response)
    }
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
    producer: &DynamicResponseProducer<ModelBackendOutput>,
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
        let response: Value = serde_json::from_str(data)?;
        if let Some(content) = response
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
            .and_then(|choice| choice.get("delta"))
            .and_then(|delta| delta.get("content"))
            .and_then(Value::as_str)
            .filter(|content| !content.is_empty())
        {
            producer.update(|output| output.content.push_str(content));
        }
    }

    Ok(false)
}
