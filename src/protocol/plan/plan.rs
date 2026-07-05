use crate::StepDraft;
use crate::external::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    pub description: String,
    pub ready_steps: Vec<StepDraft>,
    pub pending_steps: Vec<StepDraft>,
    pub expected_result: String,
}

impl Plan {
    pub fn parse(content: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(content)
    }
}
