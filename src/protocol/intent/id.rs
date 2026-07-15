use crate::external::*;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct IntentId(pub uuid::Uuid);

impl IntentId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}
