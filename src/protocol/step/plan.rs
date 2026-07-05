use crate::StepDraft;
use crate::external::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepPlan {
    pub description: String,
    pub ready_steps: Vec<StepDraft>,
    pub pending_steps: Vec<StepDraft>,
    pub expected_result: String,
}

impl StepPlan {
    pub fn parse(content: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(content)
    }
}
