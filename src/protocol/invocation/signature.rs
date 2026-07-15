use std::fmt;

use crate::external::*;

use crate::{InvocationId, Signature, StepSignature};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct InvocationSignature {
    pub step: StepSignature,
    pub id: InvocationId,
    pub name: String,
}

impl InvocationSignature {
    pub fn new(step: StepSignature, name: String) -> Self {
        Self {
            step,
            id: InvocationId::new(),
            name,
        }
    }
}

impl Signature for InvocationSignature {
    fn id(&self) -> uuid::Uuid {
        self.id.0
    }
}

impl fmt::Display for InvocationSignature {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.id.0)
    }
}
