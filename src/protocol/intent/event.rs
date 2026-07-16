use marix_common::ActorStatus;

use crate::external::*;
use crate::{PlanSignature, RelaySignature, StepSignature};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntentEvent {
    PlanUpdate(PlanSignature, ActorStatus),
    StepUpdate(StepSignature, ActorStatus),
    RelayUpdate(RelaySignature, ActorStatus),
    Cancel,
}
