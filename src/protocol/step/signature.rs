use std::fmt;

use crate::external::*;

use crate::{PlanSignature, Signature, StepId, TaskSignature};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct StepSignature {
    pub task: TaskSignature,
    pub plan: PlanSignature,
    pub id: StepId,
    pub name: String,
}

impl StepSignature {
    pub fn new(task: TaskSignature, plan: PlanSignature, name: String) -> Self {
        Self {
            task,
            plan,
            id: StepId::new(),
            name,
        }
    }
}

impl Signature for StepSignature {
    fn id(&self) -> uuid::Uuid {
        self.id.0
    }
}

impl fmt::Display for StepSignature {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.id.0)
    }
}
