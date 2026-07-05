use marix_common::System;
use marix_protocol::{TaskPreview, ToolPreview};

pub struct SessionContext {
    pub system: Option<System>,
    pub tasks: Vec<TaskPreview>,
    pub tools: Vec<ToolPreview>,
}
