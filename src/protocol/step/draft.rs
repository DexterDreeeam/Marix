use crate::external::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepDraft {
    pub name: String,
    #[serde(default)]
    pub kind: String,
    pub description: String,
    #[serde(default)]
    pub input: String,
}

impl StepDraft {
    pub fn parse(mut self, kind: &str) -> Self {
        self.kind = kind.to_owned();
        self
    }
}
