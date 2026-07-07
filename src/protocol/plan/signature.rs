use crate::external::*;
use crate::{PlanId, Signature, TaskSignature};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PlanSignature {
    pub task: TaskSignature,
    pub id: PlanId,
}

impl PlanSignature {
    pub fn new(task: TaskSignature) -> Self {
        Self {
            task,
            id: PlanId::new(),
        }
    }
}

impl Signature for PlanSignature {
    fn id(&self) -> uuid::Uuid {
        self.id.0
    }
}
