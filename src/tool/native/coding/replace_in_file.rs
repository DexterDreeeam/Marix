use std::fs;

use marix_common::{
    Arch, Platform, System,
    external::serde_json::{Value, json, to_string},
};
use marix_protocol::{ToolCategory, ToolPreview};

use super::super::parse_input;
use crate::ToolProgram;

pub struct ReplaceInFile;

impl ReplaceInFile {
    const NAME: &'static str = "replace_in_file";
    const DESCRIPTION: &'static str =
        "Replace an exact code block with a new code block in a file.";
    const INPUT_SCHEMA: &'static str = r#"{"type":"object","properties":{"path":{"type":"string"},"old_str":{"type":"string"},"new_str":{"type":"string"}},"required":["path","old_str","new_str"],"additionalProperties":false}"#;
}

impl ToolProgram for ReplaceInFile {
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
        let Some(old_str) = input.get("old_str").and_then(Value::as_str) else {
            return failure("missing required field: old_str".to_owned());
        };
        let Some(new_str) = input.get("new_str").and_then(Value::as_str) else {
            return failure("missing required field: new_str".to_owned());
        };

        let content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(error) => return failure(format!("failed to read '{path}': {error}")),
        };

        if !content.contains(old_str) {
            return failure("old_str not found in file".to_owned());
        }

        let new_content = content.replace(old_str, new_str);
        match fs::write(path, new_content) {
            Ok(_) => to_string(&json!({ "success": true })).unwrap_or_default(),
            Err(error) => failure(format!("failed to write to '{path}': {error}")),
        }
    }
}

fn failure(message: String) -> String {
    to_string(&json!({ "error": message })).unwrap_or_default()
}

#[cfg(feature = "replace_in_file")]
pub use self::ReplaceInFile as SelectedTool;
