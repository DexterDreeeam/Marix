use super::{UserMessage, UserMessageType};
use crate::common::external::*;

use super::UserMessageEnvelope;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub content: String,
}

impl UserMessage for ChatMessage {
    fn message_type(&self) -> UserMessageType {
        UserMessageType::Chat
    }

    fn into_envelope(self) -> UserMessageEnvelope {
        UserMessageEnvelope::Chat(self)
    }
}
