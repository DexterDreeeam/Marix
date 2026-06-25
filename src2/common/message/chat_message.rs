use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{ProtocolConvertError, UserMessage, UserMessageType};

const MESSAGE_TYPE_BYTES: usize = 1;
const FIELD_LENGTH_BYTES: usize = std::mem::size_of::<u64>();

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub correlation_id: String,
    pub text: String,
}

impl ChatMessage {
    pub fn new(correlation_id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            correlation_id: correlation_id.into(),
            text: text.into(),
        }
    }

    fn with_random_correlation_id(text: impl Into<String>) -> Self {
        Self::new(Uuid::new_v4().to_string(), text)
    }

    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }

    pub fn to_bytes(&self, message_type: UserMessageType) -> Result<Vec<u8>, ProtocolConvertError> {
        let mut bytes = Vec::new();
        bytes.push(message_type.code());
        write_string_field(&mut bytes, &self.correlation_id)?;
        write_string_field(&mut bytes, &self.text)?;
        Ok(bytes)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ProtocolConvertError> {
        let mut offset = MESSAGE_TYPE_BYTES;
        let correlation_id = read_string_field(bytes, &mut offset)?;
        let text = read_string_field(bytes, &mut offset)?;
        if offset != bytes.len() {
            return Err(ProtocolConvertError::PayloadLengthMismatch {
                declared: offset,
                actual: bytes.len(),
            });
        }
        Ok(Self {
            correlation_id,
            text,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessageInput {
    pub base: ChatMessage,
}

impl ChatMessageInput {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            base: ChatMessage::with_random_correlation_id(text),
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

    fn correlation_id(&self) -> &str {
        &self.base.correlation_id
    }

    fn to_bytes(&self) -> Result<Vec<u8>, ProtocolConvertError> {
        self.base.to_bytes(self.get_type())
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, ProtocolConvertError> {
        Ok(Self {
            base: ChatMessage::from_bytes(bytes)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessageOutput {
    pub base: ChatMessage,
}

impl ChatMessageOutput {
    pub fn new(correlation_id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            base: ChatMessage::new(correlation_id, text),
        }
    }

    pub fn content(&self) -> &str {
        &self.base.text
    }

    pub fn is_empty(&self) -> bool {
        self.base.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompleteMessage {
    pub correlation_id: String,
}

impl CompleteMessage {
    pub fn new(correlation_id: impl Into<String>) -> Self {
        Self {
            correlation_id: correlation_id.into(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.correlation_id.trim().is_empty()
    }
}

impl UserMessage for CompleteMessage {
    fn get_type(&self) -> UserMessageType {
        UserMessageType::CompleteMessage
    }

    fn correlation_id(&self) -> &str {
        &self.correlation_id
    }

    fn to_bytes(&self) -> Result<Vec<u8>, ProtocolConvertError> {
        let mut bytes = Vec::new();
        bytes.push(self.get_type().code());
        write_string_field(&mut bytes, &self.correlation_id)?;
        Ok(bytes)
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, ProtocolConvertError> {
        let mut offset = MESSAGE_TYPE_BYTES;
        let correlation_id = read_string_field(bytes, &mut offset)?;
        if offset != bytes.len() {
            return Err(ProtocolConvertError::PayloadLengthMismatch {
                declared: offset,
                actual: bytes.len(),
            });
        }
        Ok(Self { correlation_id })
    }
}

fn write_string_field(bytes: &mut Vec<u8>, value: &str) -> Result<(), ProtocolConvertError> {
    let field_bytes = value.as_bytes();
    let field_len = u64::try_from(field_bytes.len())
        .map_err(|_| ProtocolConvertError::PayloadTooLarge { length: u64::MAX })?;
    bytes.extend_from_slice(&field_len.to_be_bytes());
    bytes.extend_from_slice(field_bytes);
    Ok(())
}

fn read_string_field(bytes: &[u8], offset: &mut usize) -> Result<String, ProtocolConvertError> {
    if bytes.len() < *offset + FIELD_LENGTH_BYTES {
        return Err(ProtocolConvertError::MessageTooShort {
            expected_at_least: *offset + FIELD_LENGTH_BYTES,
            actual: bytes.len(),
        });
    }
    let field_len = u64::from_be_bytes(
        bytes[*offset..*offset + FIELD_LENGTH_BYTES]
            .try_into()
            .map_err(|_| ProtocolConvertError::MessageTooShort {
                expected_at_least: *offset + FIELD_LENGTH_BYTES,
                actual: bytes.len(),
            })?,
    );
    *offset += FIELD_LENGTH_BYTES;
    let field_len = usize::try_from(field_len)
        .map_err(|_| ProtocolConvertError::PayloadTooLarge { length: field_len })?;
    if bytes.len() < *offset + field_len {
        return Err(ProtocolConvertError::MessageTooShort {
            expected_at_least: *offset + field_len,
            actual: bytes.len(),
        });
    }
    let value = String::from_utf8(bytes[*offset..*offset + field_len].to_vec())
        .map_err(ProtocolConvertError::InvalidUtf8)?;
    *offset += field_len;
    Ok(value)
}

impl UserMessage for ChatMessageOutput {
    fn get_type(&self) -> UserMessageType {
        UserMessageType::ChatMessageOutput
    }

    fn correlation_id(&self) -> &str {
        &self.base.correlation_id
    }

    fn to_bytes(&self) -> Result<Vec<u8>, ProtocolConvertError> {
        self.base.to_bytes(self.get_type())
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, ProtocolConvertError> {
        Ok(Self {
            base: ChatMessage::from_bytes(bytes)?,
        })
    }
}
