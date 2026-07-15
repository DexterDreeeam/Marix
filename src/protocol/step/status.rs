use crate::external::*;
use crate::StepResult;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepStatus {
    Created,
    Running,
    Complete(StepResult),
}

impl StepStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Complete(_))
    }
}
