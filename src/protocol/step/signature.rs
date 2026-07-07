use crate::external::*;

use crate::{Signature, StepId, StepKind, TaskSignature};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepSignature {
    pub task: TaskSignature,
    pub id: StepId,
    pub description: String,
    pub kind: StepKind,
}

impl StepSignature {
    pub fn new(task: TaskSignature, description: String, kind: StepKind) -> Self {
        Self {
            task,
            id: StepId::new(),
            description,
            kind,
        }
    }
}

impl Signature for StepSignature {
    fn id(&self) -> uuid::Uuid {
        self.id.0
    }
}
