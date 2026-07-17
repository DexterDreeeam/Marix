use std::fmt;

use marix_common::Signature;

use crate::external::*;
use crate::{IntentSignature, PlanId};

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
    fn type_name(&self) -> &'static str {
        "plan"
    }

    fn id(&self) -> uuid::Uuid {
        self.id.0
    }
}

impl fmt::Display for PlanSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.type_name(), self.id())
    }
}
