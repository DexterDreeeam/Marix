#[cfg(windows)]
use std::env;
#[cfg(windows)]
use std::path::PathBuf;

use marix_common::{Arch, Platform, System};
use marix_protocol::{ToolCategory, ToolPreview};

use super::executor;
use crate::ToolProgram;

pub struct CommandPrompt;

impl CommandPrompt {
    const NAME: &'static str = "command_prompt";
    const DESCRIPTION: &'static str = "Execute a command using Windows Command Prompt (cmd.exe) using batch \
         syntax and Windows paths.";
    const INPUT_SCHEMA: &'static str = r#"{"type":"object","properties":{"command":{"type":"string"},"cwd":{"type":"string"},"timeout_ms":{"type":"integer","minimum":1}},"required":["command"],"additionalProperties":false}"#;
}

impl ToolProgram for CommandPrompt {
    fn preview(&self) -> ToolPreview {
        ToolPreview {
            name: Self::NAME.to_owned(),
            description: Self::DESCRIPTION.to_owned(),
            category: ToolCategory::Shell,
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
            executor::invoke(call, Self::program(), &["/D", "/S", "/C"])
        }
        #[cfg(not(windows))]
        {
            let _ = call;
            executor::unavailable(Self::NAME, "Windows")
        }
    }
}

#[cfg(feature = "command_prompt")]
pub use self::CommandPrompt as SelectedTool;

// -- Private -- //

#[cfg(windows)]
impl CommandPrompt {
    fn program() -> Result<PathBuf, String> {
        if let Some(com_spec) = env::var_os("ComSpec") {
            let path = PathBuf::from(com_spec);
            if path.is_absolute() {
                return Ok(path);
            }
            return Err("ComSpec does not contain an absolute path".to_owned());
        }

        let system_root = env::var_os("SystemRoot")
            .ok_or_else(|| "ComSpec and SystemRoot are unavailable".to_owned())?;
        let path = PathBuf::from(system_root).join("System32").join("cmd.exe");
        if !path.is_absolute() {
            return Err("SystemRoot does not contain an absolute path".to_owned());
        }
        Ok(path)
    }
}
