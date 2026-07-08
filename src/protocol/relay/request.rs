use crate::external::*;

use crate::RelaySignature;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayRequest {
    pub signature: RelaySignature,
    pub prompt: String,
}
