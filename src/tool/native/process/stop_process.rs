use marix_common::{
    Arch, Platform, System,
    external::serde_json::{Value, json, to_string},
};
use marix_protocol::{ToolCategory, ToolPreview};

use super::super::parse_input;
use super::process_registry;
use crate::ToolProgram;

pub struct StopProcess;

impl StopProcess {
    const NAME: &'static str = "stop_process";
    const DESCRIPTION: &'static str =
        "Stop a process previously registered by start_process, including its child process tree.";
    const INPUT_SCHEMA: &'static str = r#"{"type":"object","properties":{"process_id":{"type":"string","format":"uuid"}},"required":["process_id"],"additionalProperties":false}"#;
}

impl ToolProgram for StopProcess {
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
            return process_registry::stop(process_id).unwrap_or_else(Self::failure);
        }
        #[cfg(not(windows))]
        {
            let _ = call;
            Self::failure("stop_process is unavailable outside Windows".to_owned())
        }
    }
}

#[cfg(feature = "stop_process")]
pub use self::StopProcess as SelectedTool;

// -- Private -- //

impl StopProcess {
    fn failure(message: String) -> String {
        to_string(&json!({ "error": message })).unwrap_or_default()
    }
}
