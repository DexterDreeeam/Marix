use crate::external::*;

use crate::{StepEvent, StepSignature, StepStatus};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanEvent {
    Step(StepSignature, StepEvent),
    Update(StepStatus),
    Cancel,
}
