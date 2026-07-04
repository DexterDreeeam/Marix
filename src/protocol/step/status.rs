use crate::external::*;

use crate::StepResult;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepStatus {
    Prepare,
    Process,
    Complete(StepResult),
    Fail(StepResult),
}
