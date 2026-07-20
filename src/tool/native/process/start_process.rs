use marix_common::{
    Arch, Platform, System,
    external::serde_json::{Value, json, to_string},
};
use marix_protocol::{ToolCategory, ToolPreview};

use super::super::parse_input;
use super::process_registry;
use crate::ToolProgram;

pub struct StartProcess;

impl StartProcess {
    const NAME: &'static str = "start_process";
    const DESCRIPTION: &'static str =
        "Start a Windows process and register it for output reads and stopping.";
    const INPUT_SCHEMA: &'static str = r#"{"type":"object","properties":{"command":{"type":"string","minLength":1},"args":{"type":"array","items":{"type":"string"}},"cwd":{"type":"string"}},"required":["command"],"additionalProperties":false}"#;
}

impl ToolProgram for StartProcess {
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
            let Some(command) = input.get("command").and_then(Value::as_str) else {
                return Self::failure("missing required field: command".to_owned());
            };
            let args = match Self::args(&input) {
                Ok(args) => args,
                Err(error) => return Self::failure(error),
            };
            let cwd = input.get("cwd").and_then(Value::as_str);
            if let Err(error) = Self::prevent_tool_pipe_inheritance() {
                return Self::failure(error);
            }
            return process_registry::start(command, &args, cwd).unwrap_or_else(Self::failure);
        }
        #[cfg(not(windows))]
        {
            let _ = call;
            Self::failure("start_process is unavailable outside Windows".to_owned())
        }
    }
}

#[cfg(feature = "start_process")]
pub use self::StartProcess as SelectedTool;

// -- Private -- //

impl StartProcess {
    fn args(input: &Value) -> Result<Vec<String>, String> {
        let Some(args) = input.get("args") else {
            return Ok(Vec::new());
        };
        let Some(args) = args.as_array() else {
            return Err("args must be an array of strings".to_owned());
        };
        args.iter()
            .map(|value| {
                value
                    .as_str()
                    .map(ToOwned::to_owned)
                    .ok_or_else(|| "args must be an array of strings".to_owned())
            })
            .collect()
    }

    #[cfg(windows)]
    fn prevent_tool_pipe_inheritance() -> Result<(), String> {
        const HANDLE_FLAG_INHERIT: u32 = 1;
        const STD_INPUT_HANDLE: u32 = u32::MAX - 9;
        const STD_OUTPUT_HANDLE: u32 = u32::MAX - 10;
        const STD_ERROR_HANDLE: u32 = u32::MAX - 11;
        const INVALID_HANDLE_VALUE: isize = -1;

        unsafe extern "system" {
            fn GetStdHandle(standard_handle: u32) -> isize;
            fn SetHandleInformation(handle: isize, mask: u32, flags: u32) -> i32;
        }

        for standard_handle in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
            let handle = unsafe { GetStdHandle(standard_handle) };
            if handle == 0 || handle == INVALID_HANDLE_VALUE {
                continue;
            }
            if unsafe { SetHandleInformation(handle, HANDLE_FLAG_INHERIT, 0) } == 0 {
                return Err(format!(
                    "failed to prevent tool pipe inheritance: {}",
                    std::io::Error::last_os_error()
                ));
            }
        }
        Ok(())
    }

    fn failure(message: String) -> String {
        to_string(&json!({ "error": message })).unwrap_or_default()
    }
}
