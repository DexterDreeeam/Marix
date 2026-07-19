#[cfg(windows)]
use std::env;
#[cfg(windows)]
use std::path::PathBuf;

use marix_common::{Arch, Platform, System};
use marix_protocol::{ToolCategory, ToolPreview};

use super::executor;
use crate::ToolProgram;

pub struct PowerShellExec;

impl PowerShellExec {
    const NAME: &'static str = "powershell_exec";
    const DESCRIPTION: &'static str =
        "Execute a command using PowerShell on Windows using PowerShell syntax and Windows paths.";
    const INPUT_SCHEMA: &'static str = r#"{"type":"object","properties":{"command":{"type":"string"},"cwd":{"type":"string"},"timeout_ms":{"type":"integer","minimum":1}},"required":["command"],"additionalProperties":false}"#;
}

impl ToolProgram for PowerShellExec {
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
            executor::invoke(
                call,
                Self::program(),
                &["-NoProfile", "-NonInteractive", "-Command"],
            )
        }
        #[cfg(not(windows))]
        {
            let _ = call;
            executor::unavailable(Self::NAME, "Windows")
        }
    }
}

#[cfg(feature = "powershell_exec")]
pub use self::PowerShellExec as SelectedTool;

// -- Private -- //

#[cfg(windows)]
impl PowerShellExec {
    fn program() -> Result<PathBuf, String> {
        let system_root =
            env::var_os("SystemRoot").ok_or_else(|| "SystemRoot is unavailable".to_owned())?;
        let path = PathBuf::from(system_root)
            .join("System32")
            .join("WindowsPowerShell")
            .join("v1.0")
            .join("powershell.exe");
        if !path.is_absolute() {
            return Err("SystemRoot does not contain an absolute path".to_owned());
        }
        Ok(path)
    }
}
