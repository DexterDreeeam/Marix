use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserInput {
    pub chat_text: String,
}

impl UserInput {
    pub fn new(chat_text: impl Into<String>) -> Self {
        Self {
            chat_text: chat_text.into(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.chat_text.trim().is_empty()
    }
}
