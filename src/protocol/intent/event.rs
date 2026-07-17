use marix_common::ActorStatus;

use crate::external::*;
use crate::{PlanResult, PlanSignature, RelayResult, RelaySignature, StepResult, StepSignature};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntentEvent {
    PlanUpdate(PlanSignature, ActorStatus<PlanResult>),
    StepUpdate(StepSignature, ActorStatus<StepResult>),
    RelayUpdate(RelaySignature, ActorStatus<RelayResult>),
    Cancel,
}
