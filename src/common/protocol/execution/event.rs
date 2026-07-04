use crate::external::*;

use crate::protocol::{ExecutionRequest, ExecutionStatus, ToolPreview};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionEvent {
    PreviewQuery,
    Preview { tools: Vec<ToolPreview> },
    Evoke(ExecutionRequest),
    Query,
    Cancel,
    Kill,
    Status(ExecutionStatus),
    Update(ExecutionUpdate),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionUpdate {
    pub content: String,
}
