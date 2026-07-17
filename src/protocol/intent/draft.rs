use crate::external::*;
use crate::{PlanDraft, StepDraft};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent, deny_unknown_fields)]
pub struct IntentDraft {
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decision", deny_unknown_fields)]
pub enum IntentVerdict {
    #[serde(rename = "tool_calls")]
    Step(StepDraft),
    #[serde(rename = "plan")]
    Plan(PlanDraft),
    #[serde(rename = "complete")]
    Complete {
        #[serde(rename = "answer")]
        output: String,
    },
}

impl IntentVerdict {
    pub fn parse(content: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(content)
    }
}
