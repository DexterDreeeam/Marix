use crate::external::*;

use crate::{RelayRequest, RelayStatus};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelayEvent {
    Evoke(RelayRequest),
    Update(RelayUpdate),
    Status(RelayStatus),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayUpdate {
    pub seq: usize,
    pub content: String,
}
