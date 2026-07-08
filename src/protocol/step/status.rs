use crate::external::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepStatus {
    Created,
    Started,
    Canceled,
    Succeed,
    Failed,
}
