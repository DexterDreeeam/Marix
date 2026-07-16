use marix_common::ActorStatus;

use crate::external::*;

use crate::InvocationSignature;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepEvent {
    Update(InvocationSignature, ActorStatus),
    Cancel,
}
