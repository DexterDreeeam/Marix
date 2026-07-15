use std::fmt;

use crate::external::*;
use crate::{IntentId, PlanSignature, Signature, TaskSignature};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct IntentSignature {
    pub task: TaskSignature,
    pub parent: Option<PlanSignature>,
    pub id: IntentId,
    pub name: String,
}

impl IntentSignature {
    pub fn new(
        task: TaskSignature,
        parent: Option<PlanSignature>,
        name: String,
    ) -> Self {
        Self {
            task,
            parent,
            id: IntentId::new(),
            name,
        }
    }
}

impl Signature for IntentSignature {
    fn id(&self) -> uuid::Uuid {
        self.id.0
    }
}

impl fmt::Display for IntentSignature {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.id.0)
    }
}
