use marix_common::ActorStatus;

use crate::external::*;
use crate::{
    IntentEvent, IntentSignature, InvocationEvent, InvocationSignature, PlanEvent, PlanSignature,
    RelayEvent, RelaySignature, StepEvent, StepSignature,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskEvent {
    Intent(IntentSignature, IntentEvent),
    IntentStart(IntentSignature),
    Plan(PlanSignature, PlanEvent),
    Step(StepSignature, StepEvent),
    Invocation(InvocationSignature, InvocationEvent),
    Relay(RelaySignature, RelayEvent),
    Update(IntentSignature, ActorStatus),
    Cancel,
}
