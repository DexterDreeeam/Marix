use std::fs;
use std::path::Path;

use marix_common::external::serde_json::{Value, from_str, json, to_string};
use marix_common::{ToolInputSchema, ToolOutputSchema, ToolPreview, ToolSchema};

use crate::ToolProgram;

pub struct WriteFile;

impl WriteFile {
    const NAME: &'static str = "native_write_file";
    const DESCRIPTION: &'static str = "Write UTF-8 text content to a local file.";
    const INPUT_SCHEMA: &'static str = r#"{"type":"object","properties":{"path":{"type":"string"},"content":{"type":"string"},"create_dirs":{"type":"boolean"}},"required":["path","content"],"additionalProperties":false}"#;
    const OUTPUT_SCHEMA: &'static str = r#"{"type":"object","properties":{"bytes_written":{"type":"integer"}},"required":["bytes_written"],"additionalProperties":false}"#;
}

impl ToolProgram for WriteFile {
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
        let Some(content) = input.get("content").and_then(Value::as_str) else {
            return failure("missing required field: content".to_owned());
        };
        let create_dirs = input
            .get("create_dirs")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if create_dirs {
            if let Some(parent) = Path::new(path).parent() {
                if let Err(error) = fs::create_dir_all(parent) {
                    return failure(format!(
                        "failed to create directories for '{path}': {error}"
                    ));
                }
            }
        }
        match fs::write(path, content.as_bytes()) {
            Ok(()) => to_string(&json!({ "bytes_written": content.len() })).unwrap_or_default(),
            Err(error) => failure(format!("failed to write '{path}': {error}")),
        }
    }
}

fn failure(message: String) -> String {
    to_string(&json!({ "error": message })).unwrap_or_default()
}

#[cfg(feature = "write_file")]
pub use self::WriteFile as SelectedTool;
