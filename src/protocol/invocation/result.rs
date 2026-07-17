use crate::external::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvocationResultKind {
    Succeed,
    Canceled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvocationResult {
    pub kind: InvocationResultKind,
    pub output: String,
    pub seq_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallResultDraft {
    pub tool: String,
    pub result: InvocationResult,
}
