use crate::external::*;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ExeId(pub uuid::Uuid);

impl ExeId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}
