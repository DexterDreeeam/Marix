use crate::external::*;

use marix_common::System;

use crate::{ExecutionRequest, ExecutionStatus, ToolPreview};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionEvent {
    PreviewQuery,
    Preview {
        system: System,
        tools: Vec<ToolPreview>,
    },
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
