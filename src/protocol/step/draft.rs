use crate::InvocationDraft;
use crate::external::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepDraft {
    pub invocations: Vec<InvocationDraft>,
}
