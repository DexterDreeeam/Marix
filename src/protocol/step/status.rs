use crate::external::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepStatus {
    Created,
    Started,
    Canceled,
    Succeed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepletStatus {
    Created,
    Started,
    Canceled,
    Succeed { seq_count: usize },
    Failed,
}
