use marix_common::ActorStatus;

use crate::external::*;

use crate::{IntentResult, IntentSignature, RelayResult, RelaySignature};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanEvent {
    Update(IntentSignature, ActorStatus<IntentResult>),
    RelayUpdate(RelaySignature, ActorStatus<RelayResult>),
    Cancel,
}
