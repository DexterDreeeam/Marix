use crate::external::*;

use crate::{
    InvocationEvent, InvocationRequest, InvocationSignature, RelayEvent, RelayRequest,
    RelaySignature, StepletStatus,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepEvent {
    Invocation(InvocationSignature, InvocationEvent),
    InvocationCreate(InvocationRequest),
    Relay(RelaySignature, RelayEvent),
    RelayCreate(RelayRequest),
    Update(StepletStatus),
    Processing { seq: usize, content: String },
    Cancel,
}
