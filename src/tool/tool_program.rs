use marix_common::ToolPreview;

pub trait ToolProgram {
    fn preview(&self) -> ToolPreview;

    fn invoke(&self, call: &str) -> String;
}
