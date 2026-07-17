use std::fmt;

use marix_common::Signature;

use crate::external::*;
use crate::{IntentId, PlanSignature, TaskSignature};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct IntentSignature {
    pub task: TaskSignature,
    pub parent: Option<PlanSignature>,
    pub id: IntentId,
    pub name: String,
}

impl IntentSignature {
    pub fn new(task: TaskSignature, parent: Option<PlanSignature>, name: String) -> Self {
        Self {
            task,
            parent,
            id: IntentId::new(),
            name,
        }
    }
}

impl Signature for IntentSignature {
    fn type_name(&self) -> &'static str {
        "intent"
    }

    fn id(&self) -> uuid::Uuid {
        self.id.0
    }
}

impl fmt::Display for IntentSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.type_name(), self.id())
    }
}
