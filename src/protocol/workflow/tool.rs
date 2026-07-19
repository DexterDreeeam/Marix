use crate::ToolPreview;
use crate::external::*;

pub trait WorkflowTool: Sized {
    const NAME: &'static str;

    fn preview() -> ToolPreview;

    fn parse(arguments: &str) -> Result<Self, serde_json::Error>;
}
