use marix_common::{ToolPreview, ToolSchema};

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
                input: Self::INPUT_SCHEMA.to_owned(),
                output: Self::OUTPUT_SCHEMA.to_owned(),
            },
        }
    }

    fn invoke(&self, call: &str) -> String {
        panic!("not implemented")
    }
}

#[cfg(feature = "write_file")]
pub use self::WriteFile as SelectedTool;
