use crate::external::*;
use crate::RelayResult;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelayStatus {
    Created,
    Running,
    Complete(RelayResult),
}

impl RelayStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Complete(_))
    }
}
