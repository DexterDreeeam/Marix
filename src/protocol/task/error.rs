use crate::external::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskError {
    Canceled,
    Succeeded,
    Failed,
    PlanFailed,
}
