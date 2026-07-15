use crate::external::*;
use crate::{
    PlanSignature, PlanStatus, RelaySignature, RelayStatus,
    StepSignature, StepStatus,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntentEvent {
    PlanUpdate(PlanSignature, PlanStatus),
    StepUpdate(StepSignature, StepStatus),
    RelayUpdate(RelaySignature, RelayStatus),
    Cancel,
}
