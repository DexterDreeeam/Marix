use crate::external::*;

use crate::{StepResult, StepSignature};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepPreview {
    pub signature: StepSignature,
    pub result: StepResult,
}
