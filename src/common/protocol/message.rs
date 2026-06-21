use super::ProtocolConvertError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum UserMessageType {
    ChatMessageInput = 1,
    ChatMessageOutput = 2,
}

impl UserMessageType {
    pub fn classify(bytes: &[u8]) -> Result<Self, ProtocolConvertError> {
        let Some(code) = bytes.first().copied() else {
            return Err(ProtocolConvertError::EmptyMessage);
        };
        match code {
            1 => Ok(Self::ChatMessageInput),
            2 => Ok(Self::ChatMessageOutput),
            other => Err(ProtocolConvertError::UnknownMessageType(other)),
        }
    }

    pub fn code(self) -> u8 {
        self as u8
    }
}

pub trait UserMessage {
    fn get_type(&self) -> UserMessageType;

    fn to_bytes(&self) -> Result<Vec<u8>, ProtocolConvertError>;

    fn from_bytes(bytes: &[u8]) -> Result<Self, ProtocolConvertError>
    where
        Self: Sized;
}
