use crate::StepDraft;
use crate::external::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanDraft {
    pub description: String,
    pub run_steps: Vec<StepDraft>,
    pub pending_steps: Vec<StepDraft>,
    pub expected_result: String,
}
