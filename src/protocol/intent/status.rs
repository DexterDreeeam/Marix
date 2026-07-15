use crate::external::*;
use crate::IntentResult;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntentStatus {
    Created,
    Running,
    Complete(IntentResult),
}

impl IntentStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Complete(_))
    }
}
