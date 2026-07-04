use marix_protocol::{TaskPreview, ToolPreview};

pub struct SessionContext {
    pub tasks: Vec<TaskPreview>,
    pub tools: Vec<ToolPreview>,
}
