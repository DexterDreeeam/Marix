use crate::InvocationSignature;
use crate::external::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContinuationRequest {
    pub invocation: InvocationSignature,
    pub continuation_cursor: String,
}
