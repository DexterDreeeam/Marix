use crate::external::*;

use crate::{ExeId, Signature, TaskSignature};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ExecutionSignature {
    pub task: TaskSignature,
    pub exe_id: ExeId,
    pub name: String,
}

impl ExecutionSignature {
    pub fn new(task: TaskSignature, name: String) -> Self {
        Self {
            task,
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
