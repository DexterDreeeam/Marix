use super::ProtocolConvertError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum UserMessageType {
    ChatMessageInput = 1,
    ChatMessageOutput = 2,
    CompleteMessage = 3,
}

impl UserMessageType {
    pub fn classify(bytes: &[u8]) -> Result<Self, ProtocolConvertError> {
        let Some(code) = bytes.first().copied() else {
            return Err(ProtocolConvertError::EmptyMessage);
        };
        match code {
            1 => Ok(Self::ChatMessageInput),
            2 => Ok(Self::ChatMessageOutput),
            3 => Ok(Self::CompleteMessage),
            other => Err(ProtocolConvertError::UnknownMessageType(other)),
        }
    }

    pub fn code(self) -> u8 {
        self as u8
    }
}

pub trait UserMessage {
    fn get_type(&self) -> UserMessageType;

    fn correlation_id(&self) -> &str;

    fn to_bytes(&self) -> Result<Vec<u8>, ProtocolConvertError>;

    fn from_bytes(bytes: &[u8]) -> Result<Self, ProtocolConvertError>
    where
        Self: Sized;
}
