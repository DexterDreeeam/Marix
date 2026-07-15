use crate::external::*;
use crate::PlanResult;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanStatus {
    Created,
    Running,
    Complete(PlanResult),
}

impl PlanStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Complete(_))
    }
}
