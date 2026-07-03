use crate::protocol::{ExeId, TaskId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionSignature {
    pub task_id: TaskId,
    pub exe_id: ExeId,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolPreview {
    pub name: String,
    pub description: String,
    pub schema: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionSessionEvent {
    PreviewQuery,
    Preview { tools: Vec<ToolPreview> },
    ExecutionEvoke(ToolExecutionRequest),
    ExecutionQuery,
    ExecutionCancel,
    ExecutionKill,
    ExecutionStatus(ToolExecutionStatus),
    ExecutionUpdate(ToolExecutionUpdate),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolExecutionRequest {
    pub input: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolExecutionStatus {
    Started,
    Running,
    Canceled,
    Killed,
    Succeed,
    Failed { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolExecutionUpdate {
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionParameterPackage {
    pub task_id: TaskId,
    pub prompt: Option<String>,
    pub tool_request: Option<ToolExecutionRequest>,
    pub user_options: Vec<String>,
}
