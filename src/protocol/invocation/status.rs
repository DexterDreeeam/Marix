use crate::external::*;
use crate::InvocationResult;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvocationStatus {
    Created,
    Running,
    Complete(InvocationResult),
}

impl InvocationStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Complete(_))
    }
}
