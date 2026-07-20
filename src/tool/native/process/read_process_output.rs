use marix_common::{
    Arch, Platform, System,
    external::serde_json::{Value, json, to_string},
};
use marix_protocol::{ToolCategory, ToolPreview};

use super::super::parse_input;
use super::process_registry;
use crate::ToolProgram;

pub struct ReadProcessOutput;

impl ReadProcessOutput {
    const NAME: &'static str = "read_process_output";
    const DESCRIPTION: &'static str =
        "Read the captured stdout and stderr of a process started by start_process.";
    const INPUT_SCHEMA: &'static str = r#"{"type":"object","properties":{"process_id":{"type":"string","format":"uuid"},"offset":{"type":"object","properties":{"stdout":{"type":"integer","minimum":0},"stderr":{"type":"integer","minimum":0}},"additionalProperties":false},"max_bytes":{"type":"integer","minimum":1,"maximum":65536}},"required":["process_id"],"additionalProperties":false}"#;
}

impl ToolProgram for ReadProcessOutput {
    fn preview(&self) -> ToolPreview {
        ToolPreview {
            name: Self::NAME.to_owned(),
            description: Self::DESCRIPTION.to_owned(),
            category: ToolCategory::System,
            system: System {
                platform: Platform::Win,
                arch: Arch::All,
            },
            input: Self::INPUT_SCHEMA.to_owned(),
        }
    }

    fn invoke(&self, call: &str) -> String {
        #[cfg(windows)]
        {
            let input: Value = match parse_input(call) {
                Ok(value) => value,
                Err(error) => return Self::failure(format!("invalid input: {error}")),
            };
            let Some(process_id) = input.get("process_id").and_then(Value::as_str) else {
                return Self::failure("missing required field: process_id".to_owned());
            };
            let (stdout_offset, stderr_offset) = match Self::offsets(&input) {
                Ok(offsets) => offsets,
                Err(error) => return Self::failure(error),
            };
            let max_bytes = match Self::max_bytes(&input) {
                Ok(max_bytes) => max_bytes,
                Err(error) => return Self::failure(error),
            };
            return process_registry::read_output(
                process_id,
                stdout_offset,
                stderr_offset,
                max_bytes,
            )
            .unwrap_or_else(Self::failure);
        }
        #[cfg(not(windows))]
        {
            let _ = call;
            Self::failure("read_process_output is unavailable outside Windows".to_owned())
        }
    }
}

#[cfg(feature = "read_process_output")]
pub use self::ReadProcessOutput as SelectedTool;

// -- Private -- //

impl ReadProcessOutput {
    fn offsets(input: &Value) -> Result<(u64, u64), String> {
        let Some(offset) = input.get("offset") else {
            return Ok((0, 0));
        };
        let Some(offset) = offset.as_object() else {
            return Err("offset must be an object".to_owned());
        };
        let stdout = Self::offset(offset.get("stdout"), "stdout")?;
        let stderr = Self::offset(offset.get("stderr"), "stderr")?;
        Ok((stdout, stderr))
    }

    fn offset(value: Option<&Value>, stream: &str) -> Result<u64, String> {
        match value {
            Some(value) => value
                .as_u64()
                .ok_or_else(|| format!("offset.{stream} must be a non-negative integer")),
            None => Ok(0),
        }
    }

    fn max_bytes(input: &Value) -> Result<u64, String> {
        match input.get("max_bytes") {
            Some(value) => match value.as_u64() {
                Some(value) if (1..=65_536).contains(&value) => Ok(value),
                _ => Err("max_bytes must be an integer from 1 through 65536".to_owned()),
            },
            None => Ok(65_536),
        }
    }

    fn failure(message: String) -> String {
        to_string(&json!({ "error": message })).unwrap_or_default()
    }
}
