use std::collections::BTreeMap;
use std::fmt;

use marix_common::external::*;
use marix_common::{AsyncSender, Logger, build_async_channel};
use marix_protocol::{InvocationDraft, StepDraft, ToolPreview};

use super::backend::{ModelRequest, ModelResponse, ModelResponseStream};
use super::error::ModelBackendError;

#[derive(Clone)]
pub(super) struct OpenAiCore {
    provider: &'static str,
    endpoint: String,
    model: String,
    api_key: String,
    async_client: reqwest::Client,
}

impl OpenAiCore {
    pub(super) fn new(
        provider: &'static str,
        endpoint: String,
        model: String,
        api_key: String,
    ) -> Self {
        Self {
            provider,
            endpoint,
            model,
            api_key,
            async_client: reqwest::Client::new(),
        }
    }

    pub(super) fn request_stream(
        &mut self,
        request: ModelRequest,
    ) -> Result<ModelResponseStream, ModelBackendError> {
        Logger::debug(format!(
            "{} stream request: model '{}'",
            self.provider,
            self.model.trim()
        ));
        let task_id = request.relay.intent.task.id.0.to_string();
        let native_tools = request.tools.is_some();
        let raw = match self.build_payload(&request) {
            Ok(raw) => raw,
            Err(error) => {
                Logger::error_tagged(format!("[{task_id}][Request] {error}"), ["Model Relay"]);
                return Err(error);
            }
        };
        Logger::log_tagged(format!("[{task_id}][Request] {raw}"), ["Model Relay"]);
        let core = self.clone();
        let (sender, receiver) = build_async_channel();
        tokio::spawn(async move {
            if let Err(error) =
                Self::request_stream_response(core, raw, &task_id, native_tools, sender).await
            {
                Logger::error_tagged(format!("[{task_id}][Response] {error}"), ["Model Relay"]);
            }
        });

        Ok(receiver)
    }
}

// -- Private -- //

impl OpenAiCore {
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
            "model": self.model.trim(),
            "messages": messages,
            "thinking": {
                "type": "disabled"
            },
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
                "failed to serialize {} request payload: {error}",
                self.provider,
            ))
        })
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

    async fn request_stream_response(
        core: Self,
        raw: String,
        task_id: &str,
        native_tools: bool,
        sender: AsyncSender<ModelResponse>,
    ) -> Result<(), ModelBackendError> {
        let mut response = core
            .async_client
            .post(core.endpoint.trim())
            .bearer_auth(core.api_key.trim())
            .header("content-type", "application/json")
            .body(raw)
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await?;
            return Err(ModelBackendError::RequestFailed(format!(
                "{} request failed with {status}: {body}",
                core.provider
            )));
        }

        Logger::debug(format!("{} stream established", core.provider));
        Self::stream_response(core.provider, &mut response, &sender, task_id, native_tools).await
    }

    fn log_response(task_id: &str, content: &str) {
        Logger::log_tagged(format!("[{task_id}][Response] {content}"), ["Model Relay"]);
    }
}

impl OpenAiCore {
    async fn stream_response(
        provider: &'static str,
        response: &mut reqwest::Response,
        sender: &AsyncSender<ModelResponse>,
        task_id: &str,
        native_tools: bool,
    ) -> Result<(), ModelBackendError> {
        let mut pending = Vec::new();
        let mode = if native_tools {
            StreamMode::ToolCalls
        } else {
            StreamMode::Content
        };
        let mut accumulator = StreamAccumulator::new(provider, mode, task_id.to_owned());

        loop {
            while let Some(event) = Self::take_next_sse_event(provider, &mut pending)? {
                if accumulator.handle_event(&event, sender)? {
                    return Ok(());
                }
            }
            let Some(chunk) = response.chunk().await? else {
                break;
            };
            pending.extend_from_slice(chunk.as_ref());
        }

        if !pending.iter().all(|byte| byte.is_ascii_whitespace()) {
            let event = Self::decode_sse_event(provider, pending)?;
            if accumulator.handle_event(&event, sender)? {
                return Ok(());
            }
        }

        Err(ModelBackendError::InvalidResponse(format!(
            "{provider} stream ended before the [DONE] event",
        )))
    }

    fn take_next_sse_event(
        provider: &str,
        pending: &mut Vec<u8>,
    ) -> Result<Option<String>, ModelBackendError> {
        let line_feed = pending
            .windows(2)
            .position(|window| window == b"\n\n")
            .map(|index| (index, 2));
        let carriage_return = pending
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .map(|index| (index, 4));
        let (index, separator_length) = match (line_feed, carriage_return) {
            (Some(left), Some(right)) if left.0 <= right.0 => left,
            (Some(_), Some(right)) => right,
            (Some(boundary), None) | (None, Some(boundary)) => boundary,
            (None, None) => return Ok(None),
        };
        let mut event = pending
            .drain(..index + separator_length)
            .collect::<Vec<_>>();
        event.truncate(index);
        Self::decode_sse_event(provider, event).map(Some)
    }

    fn decode_sse_event(provider: &str, event: Vec<u8>) -> Result<String, ModelBackendError> {
        String::from_utf8(event).map_err(|error| {
            ModelBackendError::InvalidResponse(format!(
                "{provider} SSE event is not valid UTF-8: {error}",
            ))
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StreamMode {
    Content,
    ToolCalls,
}

struct StreamAccumulator {
    provider: &'static str,
    mode: StreamMode,
    task_id: String,
    content: String,
    seq_count: usize,
    finish_reason: Option<String>,
    tool_calls: BTreeMap<usize, ToolCallAccumulator>,
}

#[derive(Default)]
struct ToolCallAccumulator {
    id: String,
    call_type: Option<String>,
    name: String,
    arguments: String,
}

impl StreamAccumulator {
    fn new(provider: &'static str, mode: StreamMode, task_id: String) -> Self {
        Self {
            provider,
            mode,
            task_id,
            content: String::new(),
            seq_count: 0,
            finish_reason: None,
            tool_calls: BTreeMap::new(),
        }
    }

    fn handle_event(
        &mut self,
        event: &str,
        sender: &AsyncSender<ModelResponse>,
    ) -> Result<bool, ModelBackendError> {
        let data = event
            .lines()
            .filter_map(|line| line.strip_prefix("data:"))
            .map(|data| data.strip_prefix(' ').unwrap_or(data))
            .collect::<Vec<_>>()
            .join("\n");
        if data.is_empty() {
            return Ok(false);
        }
        if data.trim() == "[DONE]" {
            self.finish(sender)?;
            return Ok(true);
        }

        self.handle_chunk(&data, sender)?;
        Ok(false)
    }

    fn handle_chunk(
        &mut self,
        data: &str,
        sender: &AsyncSender<ModelResponse>,
    ) -> Result<(), ModelBackendError> {
        let value: serde_json::Value = serde_json::from_str(data).map_err(|error| {
            ModelBackendError::InvalidResponse(format!(
                "{} SSE data is not valid JSON: {error}",
                self.provider,
            ))
        })?;
        let choices = value
            .get("choices")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ModelBackendError::InvalidResponse(format!(
                    "{} SSE data has no choices array",
                    self.provider,
                ))
            })?;
        if choices.len() != 1 {
            return Err(ModelBackendError::InvalidResponse(format!(
                "{} SSE data has {} choices; expected exactly one",
                self.provider,
                choices.len(),
            )));
        }
        let choice = choices[0].as_object().ok_or_else(|| {
            ModelBackendError::InvalidResponse(format!(
                "{} SSE choices[0] is not an object",
                self.provider,
            ))
        })?;
        let delta = choice
            .get("delta")
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| {
                ModelBackendError::InvalidResponse(format!(
                    "{} SSE choices[0].delta is not an object",
                    self.provider,
                ))
            })?;
        let content = Self::optional_string(self.provider, delta.get("content"), "delta.content")?;
        let tool_deltas = Self::tool_deltas(self.provider, delta.get("tool_calls"))?;
        let finish_reason = Self::parse_finish_reason(self.provider, choice.get("finish_reason"))?;

        let has_content = content.is_some_and(|content| !content.is_empty());
        let has_tool_calls = tool_deltas.is_some_and(|calls| !calls.is_empty());
        if self.finish_reason.is_some() && (has_content || has_tool_calls) {
            return Err(ModelBackendError::InvalidResponse(format!(
                "{} emitted response data after finish_reason",
                self.provider,
            )));
        }
        if (has_content && (!self.tool_calls.is_empty() || has_tool_calls))
            || (has_tool_calls && !self.content.is_empty())
        {
            return Err(ModelBackendError::InvalidResponse(format!(
                "{} stream contains both content and tool calls",
                self.provider,
            )));
        }
        if has_tool_calls && self.mode == StreamMode::Content {
            return Err(ModelBackendError::InvalidResponse(format!(
                "{} decision stream contains unexpected tool calls",
                self.provider,
            )));
        }
        if let (Some(existing), Some(reason)) = (&self.finish_reason, finish_reason)
            && existing != reason
        {
            return Err(ModelBackendError::InvalidResponse(format!(
                "{} changed finish_reason from `{existing}` to `{reason}`",
                self.provider,
            )));
        }

        if let Some(calls) = tool_deltas {
            for call in calls {
                self.append_tool_delta(call)?;
            }
        }
        if let Some(content) = content.filter(|content| !content.is_empty()) {
            self.content.push_str(content);
            if self.mode == StreamMode::Content {
                if sender
                    .send(ModelResponse {
                        content: content.to_owned(),
                        seq: self.seq_count,
                        complete: false,
                    })
                    .is_err()
                {
                    return Ok(());
                }
                self.seq_count += 1;
            }
        }
        if let Some(reason) = finish_reason {
            self.finish_reason = Some(reason.to_owned());
        }

        Ok(())
    }

    fn optional_string<'a>(
        provider: &str,
        value: Option<&'a serde_json::Value>,
        field: &str,
    ) -> Result<Option<&'a str>, ModelBackendError> {
        match value {
            None | Some(serde_json::Value::Null) => Ok(None),
            Some(value) => value.as_str().map(Some).ok_or_else(|| {
                ModelBackendError::InvalidResponse(format!(
                    "{provider} SSE {field} is not a string",
                ))
            }),
        }
    }

    fn tool_deltas<'a>(
        provider: &str,
        value: Option<&'a serde_json::Value>,
    ) -> Result<Option<&'a [serde_json::Value]>, ModelBackendError> {
        match value {
            None | Some(serde_json::Value::Null) => Ok(None),
            Some(value) => value
                .as_array()
                .map(Vec::as_slice)
                .map(Some)
                .ok_or_else(|| {
                    ModelBackendError::InvalidResponse(format!(
                        "{provider} SSE delta.tool_calls is not an array",
                    ))
                }),
        }
    }

    fn parse_finish_reason<'a>(
        provider: &str,
        value: Option<&'a serde_json::Value>,
    ) -> Result<Option<&'a str>, ModelBackendError> {
        let Some(reason) = Self::optional_string(provider, value, "finish_reason")? else {
            return Ok(None);
        };
        match reason {
            "stop" | "tool_calls" => Ok(Some(reason)),
            _ => Err(ModelBackendError::InvalidResponse(format!(
                "{provider} returned unsupported finish_reason `{reason}`",
            ))),
        }
    }

    fn append_tool_delta(&mut self, delta: &serde_json::Value) -> Result<(), ModelBackendError> {
        let object = delta.as_object().ok_or_else(|| {
            ModelBackendError::InvalidResponse(format!(
                "{} tool call delta is not an object",
                self.provider,
            ))
        })?;
        let raw_index = object
            .get("index")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| {
                ModelBackendError::InvalidResponse(format!(
                    "{} tool call delta has no integer index",
                    self.provider,
                ))
            })?;
        let index = usize::try_from(raw_index).map_err(|_| {
            ModelBackendError::InvalidResponse(format!(
                "{} tool call index {raw_index} is out of range",
                self.provider,
            ))
        })?;
        if index >= 5 {
            return Err(ModelBackendError::InvalidResponse(format!(
                "{} tool call index {index} exceeds the five-call limit",
                self.provider,
            )));
        }
        self.tool_calls
            .entry(index)
            .or_default()
            .append(self.provider, delta, index)
    }

    fn finish(&self, sender: &AsyncSender<ModelResponse>) -> Result<(), ModelBackendError> {
        let finish_reason = self.finish_reason.as_deref().ok_or_else(|| {
            ModelBackendError::InvalidResponse(format!(
                "{} sent [DONE] without an explicit finish_reason",
                self.provider,
            ))
        })?;
        let content = match self.mode {
            StreamMode::Content => {
                if finish_reason != "stop" {
                    return Err(ModelBackendError::InvalidResponse(format!(
                        "{} content stream ended with finish_reason `{finish_reason}`",
                        self.provider,
                    )));
                }
                None
            }
            StreamMode::ToolCalls => {
                if finish_reason != "tool_calls" {
                    return Err(ModelBackendError::InvalidResponse(format!(
                        "{} tool call stream ended with finish_reason \
                         `{finish_reason}`",
                        self.provider,
                    )));
                }
                Some(self.normalize_tool_calls()?)
            }
        };
        OpenAiCore::log_response(&self.task_id, &self.response_json()?);
        if let Some(content) = content {
            if sender
                .send(ModelResponse {
                    content,
                    seq: 0,
                    complete: false,
                })
                .is_err()
            {
                return Ok(());
            }
        }
        let seq = if self.mode == StreamMode::ToolCalls {
            1
        } else {
            self.seq_count
        };
        let _ = sender.send(ModelResponse {
            content: String::new(),
            seq,
            complete: true,
        });
        Ok(())
    }

    fn response_json(&self) -> Result<String, ModelBackendError> {
        let content = (self.mode == StreamMode::Content).then_some(&self.content);
        let tool_calls = (self.mode == StreamMode::ToolCalls).then(|| self.tool_calls_json());
        let finish_reason = if self.mode == StreamMode::Content {
            "stop"
        } else {
            "tool_calls"
        };
        serde_json::to_string(&serde_json::json!({
            "object": "chat.completion",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": content,
                    "tool_calls": tool_calls
                },
                "finish_reason": finish_reason
            }]
        }))
        .map_err(ModelBackendError::from)
    }

    fn tool_calls_json(&self) -> Vec<serde_json::Value> {
        self.tool_calls
            .values()
            .map(|call| {
                serde_json::json!({
                    "id": &call.id,
                    "type": "function",
                    "function": {
                        "name": &call.name,
                        "arguments": &call.arguments
                    },
                })
            })
            .collect()
    }

    fn normalize_tool_calls(&self) -> Result<String, ModelBackendError> {
        if !(1..=5).contains(&self.tool_calls.len()) {
            return Err(ModelBackendError::InvalidResponse(format!(
                "{} returned {} native tool calls; expected one to five",
                self.provider,
                self.tool_calls.len(),
            )));
        }
        let mut invocations = Vec::with_capacity(self.tool_calls.len());
        for (expected, (index, call)) in self.tool_calls.iter().enumerate() {
            if *index != expected {
                return Err(ModelBackendError::InvalidResponse(format!(
                    "{} tool call indices are not contiguous at index {expected}",
                    self.provider,
                )));
            }
            call.validate(self.provider, *index)?;
            invocations.push(InvocationDraft {
                name: call.name.clone(),
                input: call.arguments.clone(),
            });
        }
        serde_json::to_string(&StepDraft { invocations }).map_err(|error| {
            ModelBackendError::InvalidResponse(format!(
                "failed to normalize {} native tool calls: {error}",
                self.provider,
            ))
        })
    }
}

impl ToolCallAccumulator {
    fn append(
        &mut self,
        provider: &str,
        delta: &serde_json::Value,
        index: usize,
    ) -> Result<(), ModelBackendError> {
        let delta = delta.as_object().ok_or_else(|| {
            ModelBackendError::InvalidResponse(format!(
                "{provider} tool call delta is not an object",
            ))
        })?;
        if let Some(id) =
            StreamAccumulator::optional_string(provider, delta.get("id"), "tool call id")?
        {
            self.id.push_str(id);
        }
        if let Some(call_type) =
            StreamAccumulator::optional_string(provider, delta.get("type"), "tool call type")?
        {
            match self.call_type.as_deref() {
                None => self.call_type = Some(call_type.to_owned()),
                Some(existing) if existing == call_type => {}
                Some(existing) => {
                    return Err(ModelBackendError::InvalidResponse(format!(
                        "{provider} tool call {index} changed type from \
                         `{existing}` to `{call_type}`",
                    )));
                }
            }
        }
        let function = match delta.get("function") {
            None | Some(serde_json::Value::Null) => return Ok(()),
            Some(value) => value.as_object().ok_or_else(|| {
                ModelBackendError::InvalidResponse(format!(
                    "{provider} tool call {index} function delta is not an object",
                ))
            })?,
        };
        if let Some(name) =
            StreamAccumulator::optional_string(provider, function.get("name"), "function name")?
        {
            self.name.push_str(name);
        }
        if let Some(arguments) = StreamAccumulator::optional_string(
            provider,
            function.get("arguments"),
            "function arguments",
        )? {
            self.arguments.push_str(arguments);
        }
        Ok(())
    }

    fn validate(&self, provider: &str, index: usize) -> Result<(), ModelBackendError> {
        if self.id.trim().is_empty() {
            return Err(ModelBackendError::InvalidResponse(format!(
                "{provider} tool call {index} has no id",
            )));
        }
        if self.call_type.as_deref() != Some("function") {
            return Err(ModelBackendError::InvalidResponse(format!(
                "{provider} tool call {index} has unsupported or missing type",
            )));
        }
        if self.name.trim().is_empty() {
            return Err(ModelBackendError::InvalidResponse(format!(
                "{provider} tool call {index} has no function name",
            )));
        }
        let arguments: serde_json::Value =
            serde_json::from_str(&self.arguments).map_err(|error| {
                ModelBackendError::InvalidResponse(format!(
                    "{provider} tool call {index} arguments are invalid JSON: {error}",
                ))
            })?;
        if !arguments.is_object() {
            return Err(ModelBackendError::InvalidResponse(format!(
                "{provider} tool call {index} arguments must be a JSON object",
            )));
        }
        Ok(())
    }
}

impl fmt::Debug for OpenAiCore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct(self.provider)
            .field("api_key_configured", &(!self.api_key.is_empty()))
            .field("endpoint", &self.endpoint)
            .field("model", &self.model)
            .finish()
    }
}
