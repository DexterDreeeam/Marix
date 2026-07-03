use marix_common::{ToolPreview, ToolSchema};

use crate::ToolProgram;

pub struct SearchText;

impl SearchText {
    const NAME: &'static str = "native_search_text";
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
                input: Self::INPUT_SCHEMA.to_owned(),
                output: Self::OUTPUT_SCHEMA.to_owned(),
            },
        }
    }

    fn invoke(&self, call: &str) -> String {
        panic!("not implemented")
    }
}

#[cfg(feature = "search_text")]
pub use self::SearchText as SelectedTool;
