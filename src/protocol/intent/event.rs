use marix_common::ActorStatus;

use crate::external::*;
use crate::{
    IntentResult, IntentSignature, RelayResult, RelaySignature, StepResult, StepSignature,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntentEvent {
    SubintentUpdate(IntentSignature, ActorStatus<IntentResult>),
    StepUpdate(StepSignature, ActorStatus<StepResult>),
    RelayUpdate(RelaySignature, ActorStatus<RelayResult>),
    Cancel,
}
