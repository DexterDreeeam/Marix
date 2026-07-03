use marix_common::{ToolPreview, ToolSchema};

use crate::ToolProgram;

pub struct ReadFile;

impl ReadFile {
    const NAME: &'static str = "native_read_file";
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
                input: Self::INPUT_SCHEMA.to_owned(),
                output: Self::OUTPUT_SCHEMA.to_owned(),
            },
        }
    }

    fn invoke(&self, call: &str) -> String {
        panic!("not implemented")
    }
}

#[cfg(feature = "read_file")]
pub use self::ReadFile as SelectedTool;
