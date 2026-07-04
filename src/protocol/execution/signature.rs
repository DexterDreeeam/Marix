use crate::external::*;

use crate::{ExeId, TaskId};

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
