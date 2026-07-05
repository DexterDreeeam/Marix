use crate::Signature;
use crate::external::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanSignature {
    pub id: uuid::Uuid,
}

impl PlanSignature {
    pub fn new() -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
        }
    }
}

impl Signature for PlanSignature {
    fn id(&self) -> uuid::Uuid {
        self.id
    }
}
