use marix_protocol::ToolPreview;

pub trait ToolProgram {
    fn preview(&self) -> ToolPreview;

    fn invoke(&self, call: &str) -> String;
}
