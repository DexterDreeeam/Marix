use crate::StepResult;
use crate::external::*;

use super::{IntentResult, IntentSignature};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntentContext {
    pub signature: IntentSignature,
    pub content: String,
    pub result: Option<IntentResult>,
    pub step_results: Vec<StepResult>,
}
