use crate::StepDraft;
use crate::external::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanDraft {
    pub description: String,
    pub background: String,
    pub call: Vec<StepDraft>,
    pub model: StepDraft,
    pub future: Vec<StepDraft>,
    pub expected_result: String,
}
