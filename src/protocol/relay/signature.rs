use crate::external::*;

use crate::{RelayId, Signature, TaskSignature};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RelaySignature {
    pub task: TaskSignature,
    pub relay_id: RelayId,
    pub name: String,
}

impl RelaySignature {
    pub fn new(task: TaskSignature, name: String) -> Self {
        Self {
            task,
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
