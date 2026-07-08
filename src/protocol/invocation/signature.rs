use crate::external::*;

use crate::{InvocationId, PlanSignature, Signature, StepSignature, TaskSignature};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct InvocationSignature {
    pub task: TaskSignature,
    pub plan: PlanSignature,
    pub step: StepSignature,
    pub invocation_id: InvocationId,
    pub name: String,
}

impl InvocationSignature {
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
            invocation_id: InvocationId::new(),
            name,
        }
    }
}

impl Signature for InvocationSignature {
    fn id(&self) -> uuid::Uuid {
        self.invocation_id.0
    }
}
