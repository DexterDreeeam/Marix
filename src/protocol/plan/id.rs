use crate::external::*;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PlanId(pub uuid::Uuid);

impl PlanId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}
