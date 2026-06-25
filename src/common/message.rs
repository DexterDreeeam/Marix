pub mod chat;

pub use chat::ChatMessage;

use crate::common::external::*;

pub trait UserMessage {
    fn message_type(&self) -> UserMessageType;

    fn into_envelope(self) -> UserMessageEnvelope
    where
        Self: Sized;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserMessageType {
    Chat,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserMessageEnvelope {
    Chat(ChatMessage),
}

impl UserMessage for UserMessageEnvelope {
    fn message_type(&self) -> UserMessageType {
        match self {
            Self::Chat(message) => message.message_type(),
        }
    }

    fn into_envelope(self) -> UserMessageEnvelope {
        self
    }
}
