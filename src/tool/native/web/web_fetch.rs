use std::process::Command;

use marix_common::{
    Arch, Platform, System,
    external::serde_json::{Value, json, to_string},
};
use marix_protocol::{ToolCategory, ToolPreview};

use super::super::parse_input;
use crate::ToolProgram;

pub struct WebFetch;

impl WebFetch {
    const NAME: &'static str = "web_fetch";
    const DESCRIPTION: &'static str = "Fetch a URL from the internet and return the page content. Strips excessive HTML tags to return clean markdown-like text.";
    const INPUT_SCHEMA: &'static str = r#"{"type":"object","properties":{"url":{"type":"string"}},"required":["url"],"additionalProperties":false}"#;
}

impl ToolProgram for WebFetch {
    fn preview(&self) -> ToolPreview {
        ToolPreview {
            name: Self::NAME.to_owned(),
            description: Self::DESCRIPTION.to_owned(),
            category: ToolCategory::Web,
            system: System {
                platform: Platform::All,
                arch: Arch::All,
            },
            input: Self::INPUT_SCHEMA.to_owned(),
        }
    }

    fn invoke(&self, call: &str) -> String {
        let input: Value = match parse_input(call) {
            Ok(value) => value,
            Err(error) => return failure(format!("invalid input: {error}")),
        };
        let Some(url) = input.get("url").and_then(Value::as_str) else {
            return failure("missing required field: url".to_owned());
        };

        match Command::new("curl")
            .args([
                "-sS",
                "-L",
                "--max-time",
                "60",
                "--retry",
                "3",
                "--retry-delay",
                "2",
            ])
            .arg(url)
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    let content = String::from_utf8_lossy(&output.stdout).into_owned();
                    to_string(&json!({ "content": content })).unwrap_or_default()
                } else {
                    failure(format!(
                        "curl error: {}",
                        String::from_utf8_lossy(&output.stderr)
                    ))
                }
            }
            Err(e) => failure(format!("failed to execute curl: {e}")),
        }
    }
}

fn failure(message: String) -> String {
    to_string(&json!({ "error": message })).unwrap_or_default()
}

#[cfg(feature = "web_fetch")]
pub use self::WebFetch as SelectedTool;
