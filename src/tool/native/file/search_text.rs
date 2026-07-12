use std::fs;
use std::path::Path;

use marix_common::external::serde_json::{Value, json, to_string};
use marix_protocol::{ToolInputSchema, ToolOutputSchema, ToolPreview, ToolSchema};

use super::super::parse_input;
use crate::ToolProgram;

pub struct SearchText;

impl SearchText {
    const NAME: &'static str = "search_text";
    const DESCRIPTION: &'static str = "Search text under a local directory or file path.";
    const INPUT_SCHEMA: &'static str = r#"{"type":"object","properties":{"path":{"type":"string"},"query":{"type":"string"},"case_sensitive":{"type":"boolean"}},"required":["path","query"],"additionalProperties":false}"#;
    const OUTPUT_SCHEMA: &'static str = r#"{"type":"object","properties":{"matches":{"type":"array","items":{"type":"object","properties":{"path":{"type":"string"},"line":{"type":"integer"},"text":{"type":"string"}}}}},"required":["matches"],"additionalProperties":false}"#;
}

impl ToolProgram for SearchText {
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
        let Some(query) = input.get("query").and_then(Value::as_str) else {
            return failure("missing required field: query".to_owned());
        };
        let case_sensitive = input
            .get("case_sensitive")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let mut matches = Vec::new();
        if let Err(error) = search_path(Path::new(path), query, case_sensitive, &mut matches) {
            return failure(format!("failed to search '{path}': {error}"));
        }
        to_string(&json!({ "matches": matches })).unwrap_or_default()
    }
}

fn search_path(
    path: &Path,
    query: &str,
    case_sensitive: bool,
    matches: &mut Vec<Value>,
) -> std::io::Result<()> {
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            search_path(&entry?.path(), query, case_sensitive, matches)?;
        }
        return Ok(());
    }
    let Ok(content) = fs::read_to_string(path) else {
        return Ok(());
    };
    let needle = if case_sensitive {
        query.to_owned()
    } else {
        query.to_lowercase()
    };
    for (index, line) in content.lines().enumerate() {
        let haystack = if case_sensitive {
            line.to_owned()
        } else {
            line.to_lowercase()
        };
        if haystack.contains(&needle) {
            matches.push(json!({
                "path": path.to_string_lossy(),
                "line": index + 1,
                "text": line,
            }));
        }
    }
    Ok(())
}

fn failure(message: String) -> String {
    to_string(&json!({ "error": message })).unwrap_or_default()
}

#[cfg(feature = "search_text")]
pub use self::SearchText as SelectedTool;
