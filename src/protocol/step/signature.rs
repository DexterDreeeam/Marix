use crate::external::*;

use crate::{Signature, StepKind, TaskSignature};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepSignature {
    pub task: TaskSignature,
    pub step_no: usize,
    pub description: String,
    pub kind: StepKind,
}

impl StepSignature {
    pub fn new(task: TaskSignature, step_no: usize, description: String, kind: StepKind) -> Self {
        Self {
            task,
            step_no,
            description,
            kind,
        }
    }
}

impl Signature for StepSignature {
    fn id(&self) -> uuid::Uuid {
        let key = format!("{}:{}", self.task.id(), self.step_no);
        uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, key.as_bytes())
    }
}
