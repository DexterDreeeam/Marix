use crate::external::*;

use crate::{ExeId, Signature, TaskId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionSignature {
    pub task_id: TaskId,
    pub exe_id: ExeId,
    pub name: String,
}

impl ExecutionSignature {
    pub fn new(task_id: TaskId, name: String) -> Self {
        Self {
            task_id,
            exe_id: ExeId::new(),
            name,
        }
    }
}

impl Signature for ExecutionSignature {
    fn id(&self) -> uuid::Uuid {
        self.exe_id.0
    }
}
