use crate::external::*;

use crate::{InvocationSignature, InvocationStatus};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepEvent {
    Update(InvocationSignature, InvocationStatus),
    Cancel,
}
