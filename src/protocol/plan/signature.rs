use std::fmt;

use crate::external::*;
use crate::{PlanId, Signature, TaskSignature};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PlanSignature {
    pub task: TaskSignature,
    pub id: PlanId,
    pub name: String,
}

impl PlanSignature {
    pub fn new(task: TaskSignature, name: String) -> Self {
        Self {
            task,
            id: PlanId::new(),
            name,
        }
    }
}

impl Signature for PlanSignature {
    fn id(&self) -> uuid::Uuid {
        self.id.0
    }
}

impl fmt::Display for PlanSignature {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.id.0)
    }
}
