use crate::external::*;

use crate::{InvocationSignature, ToolInputSchema};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InvocationDraft {
    pub name: String,
    pub input: ToolInputSchema,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvocationRequest {
    pub signature: InvocationSignature,
    pub input: ToolInputSchema,
}
