use crate::external::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvocationStatus {
    Created,
    Started,
    Canceled,
    Succeed { seq_count: usize },
    Failed,
}

impl InvocationStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Canceled | Self::Succeed { .. } | Self::Failed)
    }
}
