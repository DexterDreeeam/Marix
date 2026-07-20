use std::fs;

use marix_common::{
    Arch, Platform, System,
    external::serde_json::{Value, json, to_string},
};
use marix_protocol::{ToolCategory, ToolPreview};

use super::super::parse_input;
use crate::ToolProgram;

pub struct GetCodeOutline;

impl GetCodeOutline {
    const NAME: &'static str = "get_code_outline";
    const DESCRIPTION: &'static str = "Extract the core outline (classes, functions, signatures) of a source code file, omitting the implementation details.";
    const INPUT_SCHEMA: &'static str = r#"{"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false}"#;
}

impl ToolProgram for GetCodeOutline {
    fn preview(&self) -> ToolPreview {
        ToolPreview {
            name: Self::NAME.to_owned(),
            description: Self::DESCRIPTION.to_owned(),
            category: ToolCategory::Coding,
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
        let Some(path) = input.get("path").and_then(Value::as_str) else {
            return failure("missing required field: path".to_owned());
        };
        let content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(error) => return failure(format!("failed to read '{path}': {error}")),
        };

        let mut outline = String::new();
        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("fn ")
                || trimmed.starts_with("struct ")
                || trimmed.starts_with("class ")
                || trimmed.starts_with("def ")
            {
                outline.push_str(&format!("{}: {}\n", i + 1, line));
            }
        }

        if outline.is_empty() {
            to_string(&json!({ "content": "No outline found." })).unwrap_or_default()
        } else {
            to_string(&json!({ "content": outline })).unwrap_or_default()
        }
    }
}

fn failure(message: String) -> String {
    to_string(&json!({ "error": message })).unwrap_or_default()
}

#[cfg(feature = "get_code_outline")]
pub use self::GetCodeOutline as SelectedTool;
