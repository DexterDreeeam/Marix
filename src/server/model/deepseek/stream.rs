use std::collections::BTreeMap;

use marix_common::AsyncSender;
use marix_common::external::*;
use marix_protocol::{InvocationDraft, StepDraft};

use super::DeepseekBackend;
use crate::model::{ModelBackendError, ModelResponse};

impl DeepseekBackend {
    pub(super) async fn stream_response(
        response: &mut reqwest::Response,
        sender: &AsyncSender<ModelResponse>,
        native_tools: bool,
    ) -> Result<(), ModelBackendError> {
        let mut pending = Vec::new();
        let mode = if native_tools {
            StreamMode::ToolCalls
        } else {
            StreamMode::Content
        };
        let mut accumulator = StreamAccumulator::new(mode);

        loop {
            while let Some(event) = Self::take_next_sse_event(&mut pending)? {
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
            let event = Self::decode_sse_event(pending)?;
            if accumulator.handle_event(&event, sender)? {
                return Ok(());
            }
        }

        Err(ModelBackendError::InvalidResponse(
            "Deepseek stream ended before the [DONE] event".to_owned(),
        ))
    }
}

// -- Private -- //

#[derive(Clone, Copy, PartialEq, Eq)]
enum StreamMode {
    Content,
    ToolCalls,
}

struct StreamAccumulator {
    mode: StreamMode,
    content: String,
    raw_response: String,
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

impl DeepseekBackend {
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
            (Some(left), Some(right)) if left.0 <= right.0 => left,
            (Some(_), Some(right)) => right,
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
}

impl StreamAccumulator {
    fn new(mode: StreamMode) -> Self {
        Self {
            mode,
            content: String::new(),
            raw_response: String::new(),
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
        if !self.raw_response.is_empty() {
            self.raw_response.push('\n');
        }
        self.raw_response.push_str(&data);
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
                "Deepseek SSE data is not valid JSON: {error}",
            ))
        })?;
        let choices = value
            .get("choices")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ModelBackendError::InvalidResponse(
                    "Deepseek SSE data has no choices array".to_owned(),
                )
            })?;
        if choices.len() != 1 {
            return Err(ModelBackendError::InvalidResponse(format!(
                "Deepseek SSE data has {} choices; expected exactly one",
                choices.len(),
            )));
        }
        let choice = choices[0].as_object().ok_or_else(|| {
            ModelBackendError::InvalidResponse(
                "Deepseek SSE choices[0] is not an object".to_owned(),
            )
        })?;
        let delta = choice
            .get("delta")
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| {
                ModelBackendError::InvalidResponse(
                    "Deepseek SSE choices[0].delta is not an object".to_owned(),
                )
            })?;
        let content = Self::optional_string(delta.get("content"), "delta.content")?;
        let tool_deltas = Self::tool_deltas(delta.get("tool_calls"))?;
        let finish_reason = Self::parse_finish_reason(choice.get("finish_reason"))?;

        let has_content = content.is_some_and(|content| !content.is_empty());
        let has_tool_calls = tool_deltas.is_some_and(|calls| !calls.is_empty());
        if self.finish_reason.is_some() && (has_content || has_tool_calls) {
            return Err(ModelBackendError::InvalidResponse(
                "Deepseek emitted response data after finish_reason".to_owned(),
            ));
        }
        if (has_content && (!self.tool_calls.is_empty() || has_tool_calls))
            || (has_tool_calls && !self.content.is_empty())
        {
            return Err(ModelBackendError::InvalidResponse(
                "Deepseek stream contains both content and tool calls".to_owned(),
            ));
        }
        if has_tool_calls && self.mode == StreamMode::Content {
            return Err(ModelBackendError::InvalidResponse(
                "Deepseek decision stream contains unexpected tool calls".to_owned(),
            ));
        }
        if let (Some(existing), Some(reason)) = (&self.finish_reason, finish_reason)
            && existing != reason
        {
            return Err(ModelBackendError::InvalidResponse(format!(
                "Deepseek changed finish_reason from `{existing}` to `{reason}`",
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
        value: Option<&'a serde_json::Value>,
        field: &str,
    ) -> Result<Option<&'a str>, ModelBackendError> {
        match value {
            None | Some(serde_json::Value::Null) => Ok(None),
            Some(value) => value.as_str().map(Some).ok_or_else(|| {
                ModelBackendError::InvalidResponse(format!("Deepseek SSE {field} is not a string",))
            }),
        }
    }

    fn tool_deltas(
        value: Option<&serde_json::Value>,
    ) -> Result<Option<&[serde_json::Value]>, ModelBackendError> {
        match value {
            None | Some(serde_json::Value::Null) => Ok(None),
            Some(value) => value
                .as_array()
                .map(Vec::as_slice)
                .map(Some)
                .ok_or_else(|| {
                    ModelBackendError::InvalidResponse(
                        "Deepseek SSE delta.tool_calls is not an array".to_owned(),
                    )
                }),
        }
    }

    fn parse_finish_reason(
        value: Option<&serde_json::Value>,
    ) -> Result<Option<&str>, ModelBackendError> {
        let Some(reason) = Self::optional_string(value, "finish_reason")? else {
            return Ok(None);
        };
        match reason {
            "stop" | "tool_calls" => Ok(Some(reason)),
            _ => Err(ModelBackendError::InvalidResponse(format!(
                "Deepseek returned unsupported finish_reason `{reason}`",
            ))),
        }
    }

    fn append_tool_delta(&mut self, delta: &serde_json::Value) -> Result<(), ModelBackendError> {
        let object = delta.as_object().ok_or_else(|| {
            ModelBackendError::InvalidResponse(
                "Deepseek tool call delta is not an object".to_owned(),
            )
        })?;
        let raw_index = object
            .get("index")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| {
                ModelBackendError::InvalidResponse(
                    "Deepseek tool call delta has no integer index".to_owned(),
                )
            })?;
        let index = usize::try_from(raw_index).map_err(|_| {
            ModelBackendError::InvalidResponse(format!(
                "Deepseek tool call index {raw_index} is out of range",
            ))
        })?;
        if index >= 5 {
            return Err(ModelBackendError::InvalidResponse(format!(
                "Deepseek tool call index {index} exceeds the five-call limit",
            )));
        }
        self.tool_calls
            .entry(index)
            .or_default()
            .append(delta, index)
    }

    fn finish(&self, sender: &AsyncSender<ModelResponse>) -> Result<(), ModelBackendError> {
        let finish_reason = self.finish_reason.as_deref().ok_or_else(|| {
            ModelBackendError::InvalidResponse(
                "Deepseek sent [DONE] without an explicit finish_reason".to_owned(),
            )
        })?;
        let content = match self.mode {
            StreamMode::Content => {
                if finish_reason != "stop" {
                    return Err(ModelBackendError::InvalidResponse(format!(
                        "Deepseek content stream ended with finish_reason `{finish_reason}`",
                    )));
                }
                None
            }
            StreamMode::ToolCalls => Some(self.normalize_tool_calls()?),
        };
        DeepseekBackend::log_response(&self.raw_response);

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

    fn normalize_tool_calls(&self) -> Result<String, ModelBackendError> {
        if !(1..=5).contains(&self.tool_calls.len()) {
            return Err(ModelBackendError::InvalidResponse(format!(
                "Deepseek returned {} native tool calls; expected one to five",
                self.tool_calls.len(),
            )));
        }
        let mut invocations = Vec::with_capacity(self.tool_calls.len());
        for (expected, (index, call)) in self.tool_calls.iter().enumerate() {
            if *index != expected {
                return Err(ModelBackendError::InvalidResponse(format!(
                    "Deepseek tool call indices are not contiguous at index {expected}",
                )));
            }
            call.validate(*index)?;
            invocations.push(InvocationDraft {
                name: call.name.clone(),
                input: call.arguments.clone(),
            });
        }
        serde_json::to_string(&StepDraft { invocations }).map_err(|error| {
            ModelBackendError::InvalidResponse(format!(
                "failed to normalize Deepseek native tool calls: {error}",
            ))
        })
    }
}

impl ToolCallAccumulator {
    fn append(&mut self, delta: &serde_json::Value, index: usize) -> Result<(), ModelBackendError> {
        let delta = delta.as_object().ok_or_else(|| {
            ModelBackendError::InvalidResponse(
                "Deepseek tool call delta is not an object".to_owned(),
            )
        })?;
        if let Some(id) = StreamAccumulator::optional_string(delta.get("id"), "tool call id")? {
            self.id.push_str(id);
        }
        if let Some(call_type) =
            StreamAccumulator::optional_string(delta.get("type"), "tool call type")?
        {
            match self.call_type.as_deref() {
                None => self.call_type = Some(call_type.to_owned()),
                Some(existing) if existing == call_type => {}
                Some(existing) => {
                    return Err(ModelBackendError::InvalidResponse(format!(
                        "Deepseek tool call {index} changed type from \
                         `{existing}` to `{call_type}`",
                    )));
                }
            }
        }
        let function = match delta.get("function") {
            None | Some(serde_json::Value::Null) => return Ok(()),
            Some(value) => value.as_object().ok_or_else(|| {
                ModelBackendError::InvalidResponse(format!(
                    "Deepseek tool call {index} function delta is not an object",
                ))
            })?,
        };
        if let Some(name) =
            StreamAccumulator::optional_string(function.get("name"), "function name")?
        {
            self.name.push_str(name);
        }
        if let Some(arguments) =
            StreamAccumulator::optional_string(function.get("arguments"), "function arguments")?
        {
            self.arguments.push_str(arguments);
        }
        Ok(())
    }

    fn validate(&self, index: usize) -> Result<(), ModelBackendError> {
        if self.id.trim().is_empty() {
            return Err(ModelBackendError::InvalidResponse(format!(
                "Deepseek tool call {index} has no id",
            )));
        }
        if self.call_type.as_deref() != Some("function") {
            return Err(ModelBackendError::InvalidResponse(format!(
                "Deepseek tool call {index} has unsupported or missing type",
            )));
        }
        if self.name.trim().is_empty() {
            return Err(ModelBackendError::InvalidResponse(format!(
                "Deepseek tool call {index} has no function name",
            )));
        }
        let arguments: serde_json::Value =
            serde_json::from_str(&self.arguments).map_err(|error| {
                ModelBackendError::InvalidResponse(format!(
                    "Deepseek tool call {index} arguments are invalid JSON: {error}",
                ))
            })?;
        if !arguments.is_object() {
            return Err(ModelBackendError::InvalidResponse(format!(
                "Deepseek tool call {index} arguments must be a JSON object",
            )));
        }
        Ok(())
    }
}
