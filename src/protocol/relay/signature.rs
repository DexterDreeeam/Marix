use std::fmt;

use marix_common::Signature;

use crate::external::*;

use crate::{IntentSignature, PlanSignature, RelayId};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RelaySignature {
    pub intent: IntentSignature,
    pub plan: Option<PlanSignature>,
    pub id: RelayId,
    pub name: String,
}

impl RelaySignature {
    pub fn new(intent: IntentSignature, plan: Option<PlanSignature>, name: String) -> Self {
        Self {
            intent,
            plan,
            id: RelayId::new(),
            name,
        }
    }
}

impl Signature for RelaySignature {
    fn type_name(&self) -> &'static str {
        "relay"
    }

    fn id(&self) -> uuid::Uuid {
        self.id.0
    }
}

impl fmt::Display for RelaySignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.type_name(), self.id())
    }
}
