use std::fmt;

use crate::external::*;

use crate::{IntentSignature, RelayId, Signature};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RelaySignature {
    pub intent: IntentSignature,
    pub id: RelayId,
    pub name: String,
}

impl RelaySignature {
    pub fn new(intent: IntentSignature, name: String) -> Self {
        Self {
            intent,
            id: RelayId::new(),
            name,
        }
    }
}

impl Signature for RelaySignature {
    fn id(&self) -> uuid::Uuid {
        self.id.0
    }
}

impl fmt::Display for RelaySignature {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.id.0)
    }
}
