use crate::external::*;

use crate::{PlanSignature, RelayId, Signature, StepSignature, TaskSignature};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RelaySignature {
    pub task: TaskSignature,
    pub plan: PlanSignature,
    pub step: StepSignature,
    pub relay_id: RelayId,
    pub name: String,
}

impl RelaySignature {
    pub fn new(
        task: TaskSignature,
        plan: PlanSignature,
        step: StepSignature,
        name: String,
    ) -> Self {
        Self {
            task,
            plan,
            step,
            relay_id: RelayId::new(),
            name,
        }
    }
}

impl Signature for RelaySignature {
    fn id(&self) -> uuid::Uuid {
        self.relay_id.0
    }
}
