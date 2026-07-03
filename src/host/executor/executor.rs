use marix_common::{
    ExeId, ExecutionRequest, ExecutionSignature, ExecutionStatus, ExecutionUpdate, Receiver,
    ToolPreview, WorkQueue,
};

use crate::executor::ExecutionRuntime;

pub struct Executor {
    executions: WorkQueue<ExeId, ExecutionRuntime>,
}

impl Executor {
    pub fn new() -> Self {
        panic!("not implemented")
    }

    pub fn preview(&self) -> Vec<ToolPreview> {
        panic!("not implemented")
    }

    pub fn evoke(&mut self, request: ExecutionRequest) -> ExecutionStatus {
        panic!("not implemented")
    }

    pub fn query(&self, signature: ExecutionSignature) -> ExecutionStatus {
        panic!("not implemented")
    }

    pub fn cancel(&mut self, signature: ExecutionSignature) -> ExecutionStatus {
        panic!("not implemented")
    }

    pub fn kill(&mut self, signature: ExecutionSignature) -> ExecutionStatus {
        panic!("not implemented")
    }

    pub fn updates(&self) -> Receiver<ExecutionUpdate> {
        panic!("not implemented")
    }
}
