use crate::protocol::{ExeId, TaskId};
use crate::tool::ToolPreview;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionSignature {
    pub task_id: TaskId,
    pub exe_id: ExeId,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionSessionEvent {
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
pub struct ExecutionRequest {
    pub input: String,
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
pub struct ExecutionParameterPackage {
    pub task_id: TaskId,
    pub prompt: Option<String>,
    pub tool_request: Option<ExecutionRequest>,
    pub user_options: Vec<String>,
}
