use crate::external::*;

use crate::{StepDraft, StepEvent, StepSignature, StepStatus};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanEvent {
    Step(StepSignature, StepEvent),
    StepCreate(StepDraft),
    StepUpdate(StepStatus),
    Cancel,
}
