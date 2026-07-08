use crate::external::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvocationStatus {
    Created,
    Started,
    Processing { seq: usize, content: String },
    Canceled,
    Succeed { seq_count: usize },
    Failed,
}
