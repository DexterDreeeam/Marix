use std::process::Command;

use marix_common::external::serde_json::{Value, from_str, json, to_string};
use marix_protocol::{ToolInputSchema, ToolOutputSchema, ToolPreview, ToolSchema};

use crate::ToolProgram;

pub struct ShellExecute;

impl ShellExecute {
    const NAME: &'static str = "native_shell_execute";
    const DESCRIPTION: &'static str =
        "Run a native command through the current operating system shell.";
    const INPUT_SCHEMA: &'static str = r#"{"type":"object","properties":{"command":{"type":"string"},"cwd":{"type":"string"},"timeout_ms":{"type":"integer","minimum":1}},"required":["command"],"additionalProperties":false}"#;
    const OUTPUT_SCHEMA: &'static str = r#"{"type":"object","properties":{"exit_code":{"type":"integer"},"stdout":{"type":"string"},"stderr":{"type":"string"}},"required":["exit_code"],"additionalProperties":false}"#;
}

impl ToolProgram for ShellExecute {
    fn preview(&self) -> ToolPreview {
        ToolPreview {
            name: Self::NAME.to_owned(),
            description: Self::DESCRIPTION.to_owned(),
            schema: ToolSchema {
                input: ToolInputSchema {
                    content: Self::INPUT_SCHEMA.to_owned(),
                },
                output: ToolOutputSchema {
                    content: Self::OUTPUT_SCHEMA.to_owned(),
                },
            },
        }
    }

    fn invoke(&self, call: &str) -> String {
        let input: Value = match from_str(call) {
            Ok(value) => value,
            Err(error) => return failure(format!("invalid input: {error}")),
        };
        let Some(command) = input.get("command").and_then(Value::as_str) else {
            return failure("missing required field: command".to_owned());
        };
        let cwd = input.get("cwd").and_then(Value::as_str);

        #[cfg(target_os = "windows")]
        let (program, flag) = ("cmd", "/C");
        #[cfg(not(target_os = "windows"))]
        let (program, flag) = ("sh", "-c");

        let mut process = Command::new(program);
        process.arg(flag).arg(command);
        if let Some(cwd) = cwd {
            process.current_dir(cwd);
        }
        match process.output() {
            Ok(output) => to_string(&json!({
                "exit_code": output.status.code().unwrap_or(-1),
                "stdout": String::from_utf8_lossy(&output.stdout).into_owned(),
                "stderr": String::from_utf8_lossy(&output.stderr).into_owned(),
            }))
            .unwrap_or_default(),
            Err(error) => failure(format!("failed to run command: {error}")),
        }
    }
}

fn failure(message: String) -> String {
    to_string(&json!({ "error": message })).unwrap_or_default()
}

#[cfg(feature = "shell_execute")]
pub use self::ShellExecute as SelectedTool;
