use crate::external::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionStatus {
    Created,
    Started,
    Canceled,
    Succeed { seq_count: usize },
    Failed,
}
