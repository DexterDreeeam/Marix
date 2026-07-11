use std::fs;

use marix_common::external::serde_json::{Value, from_str, json, to_string};
use marix_protocol::{ToolInputSchema, ToolOutputSchema, ToolPreview, ToolSchema};

use crate::ToolProgram;

pub struct ReadFile;

impl ReadFile {
    const NAME: &'static str = "read_file";
    const DESCRIPTION: &'static str = "Read a UTF-8 text file from the local file system.";
    const INPUT_SCHEMA: &'static str = r#"{"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false}"#;
    const OUTPUT_SCHEMA: &'static str = r#"{"type":"object","properties":{"content":{"type":"string"}},"required":["content"],"additionalProperties":false}"#;
}

impl ToolProgram for ReadFile {
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
        let Some(path) = input.get("path").and_then(Value::as_str) else {
            return failure("missing required field: path".to_owned());
        };
        match fs::read_to_string(path) {
            Ok(content) => to_string(&json!({ "content": content })).unwrap_or_default(),
            Err(error) => failure(format!("failed to read '{path}': {error}")),
        }
    }
}

fn failure(message: String) -> String {
    to_string(&json!({ "error": message })).unwrap_or_default()
}

#[cfg(feature = "read_file")]
pub use self::ReadFile as SelectedTool;
