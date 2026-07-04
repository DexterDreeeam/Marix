use crate::StepKind;
use crate::external::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepDraft {
    pub kind: StepKind,
    pub description: String,
}
