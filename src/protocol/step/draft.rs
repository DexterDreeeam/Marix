use crate::InvocationDraft;
use crate::external::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepDraft {
    #[serde(rename = "calls")]
    pub invocations: Vec<InvocationDraft>,
}
