use std::fmt;

use crate::external::*;
use crate::{IntentSignature, PlanId, Signature};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PlanSignature {
    pub intent: Box<IntentSignature>,
    pub id: PlanId,
    pub name: String,
}

impl PlanSignature {
    pub fn new(intent: IntentSignature, name: String) -> Self {
        Self {
            intent: Box::new(intent),
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
