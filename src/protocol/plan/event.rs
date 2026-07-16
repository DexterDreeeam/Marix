use marix_common::ActorStatus;

use crate::external::*;

use crate::{IntentSignature, RelaySignature};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanEvent {
    Update(IntentSignature, ActorStatus),
    RelayUpdate(RelaySignature, ActorStatus),
    Cancel,
}
