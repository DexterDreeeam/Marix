use std::fmt;

use crate::external::*;

use crate::{IntentSignature, Signature, StepId};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct StepSignature {
    pub intent: IntentSignature,
    pub id: StepId,
    pub name: String,
}

impl StepSignature {
    pub fn new(intent: IntentSignature, name: String) -> Self {
        Self {
            intent,
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
