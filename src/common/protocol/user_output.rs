use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserOutput {
    pub content: String,
}

impl UserOutput {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.content.trim().is_empty()
    }
}
