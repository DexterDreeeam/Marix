use crate::external::*;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct StepId(pub uuid::Uuid);

impl StepId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}
