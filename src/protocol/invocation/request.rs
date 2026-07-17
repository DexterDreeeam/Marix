use crate::external::*;

use crate::{InvocationSignature, ToolInputSchema};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InvocationDraft {
    #[serde(rename = "tool")]
    pub name: String,
    #[serde(rename = "arguments")]
    pub input: ToolInputSchema,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvocationRequest {
    pub signature: InvocationSignature,
    pub input: ToolInputSchema,
}
