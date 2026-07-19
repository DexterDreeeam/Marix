use marix_common::ActorStatus;

use crate::external::*;
use crate::{
    IntentEvent, IntentResult, IntentSignature, InvocationEvent, InvocationSignature, RelayEvent,
    RelaySignature, StepEvent, StepSignature,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskEvent {
    Intent(IntentSignature, IntentEvent),
    IntentStart(IntentSignature),
    Step(StepSignature, StepEvent),
    Invocation(InvocationSignature, InvocationEvent),
    Relay(RelaySignature, RelayEvent),
    Update(IntentSignature, ActorStatus<IntentResult>),
    Cancel,
}
