use crate::external::*;

use crate::{RelayKind, RelaySignature};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayRequest {
    pub signature: RelaySignature,
    pub kind: RelayKind,
}
