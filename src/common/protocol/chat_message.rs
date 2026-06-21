use serde::{Deserialize, Serialize};

use super::{ProtocolConvertError, UserMessage, UserMessageType};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessageBase {
    pub text: String,
}

impl ChatMessageBase {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }

    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }

    pub fn to_bytes(&self, message_type: UserMessageType) -> Result<Vec<u8>, ProtocolConvertError> {
        let text_bytes = self.text.as_bytes();
        let text_len = u64::try_from(text_bytes.len())
            .map_err(|_| ProtocolConvertError::PayloadTooLarge { length: u64::MAX })?;
        let mut offset = 0;
        let mut bytes = vec![0u8; 1 + 8 + text_bytes.len()];
        bytes[offset] = message_type.code();
        offset += 1;
        let length_bytes = text_len.to_be_bytes();
        bytes[offset..offset + length_bytes.len()].copy_from_slice(&length_bytes);
        offset += length_bytes.len();
        bytes[offset..].copy_from_slice(text_bytes);
        Ok(bytes)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ProtocolConvertError> {
        let mut offset = 1;
        let length_bytes = std::mem::size_of::<u64>();
        if bytes.len() < offset + length_bytes {
            return Err(ProtocolConvertError::MessageTooShort {
                expected_at_least: offset + length_bytes,
                actual: bytes.len(),
            });
        }

        let text_len = u64::from_be_bytes(
            bytes[offset..offset + length_bytes]
                .try_into()
                .map_err(|_| ProtocolConvertError::MessageTooShort {
                    expected_at_least: offset + length_bytes,
                    actual: bytes.len(),
                })?,
        );
        offset += length_bytes;
        let text_len = usize::try_from(text_len)
            .map_err(|_| ProtocolConvertError::PayloadTooLarge { length: text_len })?;

        let actual_len = bytes.len().saturating_sub(offset);
        if actual_len != text_len {
            return Err(ProtocolConvertError::PayloadLengthMismatch {
                declared: text_len,
                actual: actual_len,
            });
        }

        Ok(Self {
            text: String::from_utf8(bytes[offset..].to_vec())
                .map_err(ProtocolConvertError::InvalidUtf8)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessageInput {
    pub base: ChatMessageBase,
}

impl ChatMessageInput {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            base: ChatMessageBase::new(text),
        }
    }

    pub fn chat_text(&self) -> &str {
        &self.base.text
    }

    pub fn is_empty(&self) -> bool {
        self.base.is_empty()
    }
}

impl UserMessage for ChatMessageInput {
    fn get_type(&self) -> UserMessageType {
        UserMessageType::ChatMessageInput
    }

    fn to_bytes(&self) -> Result<Vec<u8>, ProtocolConvertError> {
        self.base.to_bytes(self.get_type())
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, ProtocolConvertError> {
        Ok(Self {
            base: ChatMessageBase::from_bytes(bytes)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessageOutput {
    pub base: ChatMessageBase,
}

impl ChatMessageOutput {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            base: ChatMessageBase::new(text),
        }
    }

    pub fn content(&self) -> &str {
        &self.base.text
    }

    pub fn is_empty(&self) -> bool {
        self.base.is_empty()
    }
}

impl UserMessage for ChatMessageOutput {
    fn get_type(&self) -> UserMessageType {
        UserMessageType::ChatMessageOutput
    }

    fn to_bytes(&self) -> Result<Vec<u8>, ProtocolConvertError> {
        self.base.to_bytes(self.get_type())
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, ProtocolConvertError> {
        Ok(Self {
            base: ChatMessageBase::from_bytes(bytes)?,
        })
    }
}
