use std::collections::BTreeMap;

use marix_common::{
    ExecutionSignature, Receiver, ToolExecutionRequest, ToolExecutionStatus, ToolExecutionUpdate,
    ToolPreview,
};

pub struct Executor {
    tools: BTreeMap<String, ToolProgram>,
    executions: BTreeMap<ExecutionSignature, ToolExecution>,
}

impl Executor {
    pub fn new() -> Self {
        panic!("not implemented")
    }

    pub fn preview(&self) -> Vec<ToolPreview> {
        panic!("not implemented")
    }

    pub fn evoke(&mut self, request: ToolExecutionRequest) -> ToolExecutionStatus {
        panic!("not implemented")
    }

    pub fn query(&self, signature: ExecutionSignature) -> ToolExecutionStatus {
        panic!("not implemented")
    }

    pub fn cancel(&mut self, signature: ExecutionSignature) -> ToolExecutionStatus {
        panic!("not implemented")
    }

    pub fn kill(&mut self, signature: ExecutionSignature) -> ToolExecutionStatus {
        panic!("not implemented")
    }

    pub fn updates(&self) -> Receiver<ToolExecutionUpdate> {
        panic!("not implemented")
    }
}

pub struct ToolProgram {
    pub name: String,
    pub executable: String,
    pub preview: ToolPreview,
}

pub struct ToolExecution {
    pub signature: ExecutionSignature,
    pub status: ToolExecutionStatus,
    pub process_id: Option<u32>,
}
