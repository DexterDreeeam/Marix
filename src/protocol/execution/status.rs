use crate::external::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionStatus {
    Started,
    Running,
    Canceled,
    Killed,
    Succeed(usize),
    Failed { reason: String },
}
