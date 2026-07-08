use crate::external::*;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ExecutionId(pub uuid::Uuid);

impl ExecutionId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}
