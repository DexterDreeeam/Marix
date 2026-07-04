use crate::external::*;

use crate::protocol::StepKind;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepSignature {
    pub step_no: usize,
    pub name: String,
    pub kind: StepKind,
}
