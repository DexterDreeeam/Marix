use crate::IntentDraft;
use crate::external::*;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decision", deny_unknown_fields)]
pub enum PlanVerdict {
    #[serde(rename = "retry")]
    Replacement(PlanDraft),
    #[serde(rename = "impossible")]
    Infeasible { reason: String },
}

impl PlanVerdict {
    pub fn parse(content: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(content)
    }
}
