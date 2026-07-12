use std::fs;
use std::path::Path;

use marix_common::external::serde_json::{Value, json, to_string};
use marix_protocol::{ToolInputSchema, ToolOutputSchema, ToolPreview, ToolSchema};

use super::super::parse_input;
use crate::ToolProgram;

pub struct ListDirectory;

impl ListDirectory {
    const NAME: &'static str = "list_directory";
    const DESCRIPTION: &'static str = "List entries under a local directory.";
    const INPUT_SCHEMA: &'static str = r#"{"type":"object","properties":{"path":{"type":"string"},"recursive":{"type":"boolean"}},"required":["path"],"additionalProperties":false}"#;
    const OUTPUT_SCHEMA: &'static str = r#"{"type":"object","properties":{"entries":{"type":"array","items":{"type":"string"}}},"required":["entries"],"additionalProperties":false}"#;
}

impl ToolProgram for ListDirectory {
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
        let input: Value = match parse_input(call) {
            Ok(value) => value,
            Err(error) => return failure(format!("invalid input: {error}")),
        };
        let Some(path) = input.get("path").and_then(Value::as_str) else {
            return failure("missing required field: path".to_owned());
        };
        let recursive = input
            .get("recursive")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let mut entries = Vec::new();
        if let Err(error) = collect_entries(Path::new(path), recursive, &mut entries) {
            return failure(format!("failed to list '{path}': {error}"));
        }
        to_string(&json!({ "entries": entries })).unwrap_or_default()
    }
}

fn collect_entries(
    directory: &Path,
    recursive: bool,
    entries: &mut Vec<String>,
) -> std::io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        entries.push(path.to_string_lossy().into_owned());
        if recursive && entry.file_type()?.is_dir() {
            collect_entries(&path, true, entries)?;
        }
    }
    Ok(())
}

fn failure(message: String) -> String {
    to_string(&json!({ "error": message })).unwrap_or_default()
}

#[cfg(feature = "list_directory")]
pub use self::ListDirectory as SelectedTool;
