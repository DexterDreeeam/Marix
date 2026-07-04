use crate::external::*;
use crate::{ExecutionSignature, ToolInputSchema};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionRequest {
    pub signature: ExecutionSignature,
    pub input: ToolInputSchema,
}
