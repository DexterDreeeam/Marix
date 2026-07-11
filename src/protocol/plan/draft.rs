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

impl PlanDraft {
    pub fn parse(content: &str) -> Result<Self, serde_json::Error> {
        let mut draft = serde_json::from_str::<Self>(content)?;
        draft.call = draft
            .call
            .into_iter()
            .map(|draft| draft.parse("tool"))
            .collect();
        draft.model = draft.model.parse("model");
        draft.future = draft
            .future
            .into_iter()
            .map(|draft| draft.parse("intent"))
            .collect();
        Ok(draft)
    }
}
