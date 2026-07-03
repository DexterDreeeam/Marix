use crate::protocol::{ExeId, TaskId, ToolPreview};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionSignature {
    pub task_id: TaskId,
    pub exe_id: ExeId,
    pub name: String,
}

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
pub enum ExecutionStatus {
    Started,
    Running,
    Canceled,
    Killed,
    Succeed,
    Failed { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionUpdate {
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionRequest {
    pub signature: ExecutionSignature,
    pub prompt: Option<String>,
    pub tool_request: Option<String>,
    pub user_options: Vec<String>,
}
