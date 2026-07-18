use marix_common::{Arch, Platform, System};
use marix_protocol::{ToolCategory, ToolPreview};

use super::executor;
use crate::ToolProgram;

pub struct Bash;

impl Bash {
    const NAME: &'static str = "bash";
    const DESCRIPTION: &'static str = "Execute a command using Bash on Unix-like systems. Use Bash/Unix \
         syntax and Unix paths; do not use PowerShell or cmd.exe syntax.";
    const INPUT_SCHEMA: &'static str = r#"{"type":"object","properties":{"command":{"type":"string"},"cwd":{"type":"string"},"timeout_ms":{"type":"integer","minimum":1}},"required":["command"],"additionalProperties":false}"#;
}

impl ToolProgram for Bash {
    fn preview(&self) -> ToolPreview {
        ToolPreview {
            name: Self::NAME.to_owned(),
            description: Self::DESCRIPTION.to_owned(),
            category: ToolCategory::Shell,
            system: System {
                platform: Platform::Ubuntu,
                arch: Arch::All,
            },
            input: Self::INPUT_SCHEMA.to_owned(),
        }
    }

    fn invoke(&self, call: &str) -> String {
        #[cfg(unix)]
        {
            executor::invoke(call, "bash", &["-lc"])
        }
        #[cfg(not(unix))]
        {
            let _ = call;
            executor::unavailable(Self::NAME, "a Unix-like operating system")
        }
    }
}

#[cfg(feature = "bash")]
pub use self::Bash as SelectedTool;
