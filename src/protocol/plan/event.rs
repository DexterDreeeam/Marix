use crate::external::*;

use crate::{IntentSignature, IntentStatus, RelaySignature, RelayStatus};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanEvent {
    Update(IntentSignature, IntentStatus),
    RelayUpdate(RelaySignature, RelayStatus),
    Cancel,
}
