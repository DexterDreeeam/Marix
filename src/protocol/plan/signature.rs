use crate::Signature;
use crate::TaskSignature;
use crate::external::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanSignature {
    pub task: TaskSignature,
    pub id: uuid::Uuid,
}

impl PlanSignature {
    pub fn new(task: TaskSignature) -> Self {
        Self {
            task,
            id: uuid::Uuid::new_v4(),
        }
    }
}

impl Signature for PlanSignature {
    fn id(&self) -> uuid::Uuid {
        self.id
    }
}
