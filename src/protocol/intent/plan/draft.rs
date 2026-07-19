use crate::external::*;

use super::super::IntentDraft;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanDraft {
    #[serde(rename = "goals")]
    pub intents: Vec<IntentDraft>,
}

impl PlanDraft {
    pub fn parse(content: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(content)
    }
}
