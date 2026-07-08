use crate::external::*;

use crate::{
    InvocationEvent, InvocationRequest, InvocationSignature, InvocationStatus, RelayEvent,
    RelayRequest, RelaySignature, RelayStatus,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepEvent {
    Invocation(InvocationSignature, InvocationEvent),
    InvocationCreate(InvocationRequest),
    InvocationUpdate(InvocationStatus),
    Relay(RelaySignature, RelayEvent),
    RelayCreate(RelayRequest),
    RelayUpdate(RelayStatus),
    Cancel,
}
