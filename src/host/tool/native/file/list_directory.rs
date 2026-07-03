use marix_common::{ToolPreview, ToolSchema};

use crate::ToolProgram;

pub struct ListDirectory;

impl ListDirectory {
    const NAME: &'static str = "native_list_directory";
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
                input: Self::INPUT_SCHEMA.to_owned(),
                output: Self::OUTPUT_SCHEMA.to_owned(),
            },
        }
    }

    fn invoke(&self, call: &str) -> String {
        panic!("not implemented")
    }
}

#[cfg(feature = "list_directory")]
pub use self::ListDirectory as SelectedTool;
