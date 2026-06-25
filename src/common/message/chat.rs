use super::{UserMessage, UserMessageType};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    pub content: String,
}

impl UserMessage for ChatMessage {
    fn message_type(&self) -> UserMessageType {
        panic!("not implemented")
    }
}
