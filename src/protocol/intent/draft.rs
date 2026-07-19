use crate::external::*;

use super::PlanDraft;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent, deny_unknown_fields)]
pub struct IntentDraft {
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decision", deny_unknown_fields)]
pub enum IntentVerdict {
    #[serde(rename = "plan")]
    Plan(PlanDraft),
    #[serde(rename = "complete")]
    Complete {
        #[serde(rename = "answer")]
        output: String,
    },
    #[serde(rename = "infeasible")]
    Infeasible { reason: String },
}

impl IntentVerdict {
    pub fn parse(content: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(content)
    }
}
