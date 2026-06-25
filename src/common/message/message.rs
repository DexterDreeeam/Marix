pub trait UserMessage {
    fn message_type(&self) -> UserMessageType;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserMessageType {
    Chat,
}
