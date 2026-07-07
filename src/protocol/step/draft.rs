use crate::external::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepDraft {
    pub name: String,
    pub kind: String,
    pub description: String,
    #[serde(default)]
    pub input: String,
}
