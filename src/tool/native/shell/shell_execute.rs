use marix_common::{ToolPreview, ToolSchema};

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
                input: Self::INPUT_SCHEMA.to_owned(),
                output: Self::OUTPUT_SCHEMA.to_owned(),
            },
        }
    }

    fn invoke(&self, call: &str) -> String {
        panic!("not implemented")
    }
}

#[cfg(feature = "shell_execute")]
pub use self::ShellExecute as SelectedTool;
