use crate::external::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionResultKind {
    Succeed,
    Canceled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub kind: ExecutionResultKind,
    pub output: String,
    pub seq_count: usize,
}
