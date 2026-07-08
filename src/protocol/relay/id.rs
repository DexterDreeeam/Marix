use crate::external::*;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RelayId(pub uuid::Uuid);

impl RelayId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}
