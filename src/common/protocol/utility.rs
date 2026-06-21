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

    fn from_bytes(bytes: &[u8]) -> Result<Self, ProtocolConvertError>;
}

#[derive(Debug)]
pub enum ProtocolConvertError {
    EmptyMessage,
    UnknownMessageType(u8),
    MessageTooShort {
        expected_at_least: usize,
        actual: usize,
    },
    PayloadTooLarge {
        length: u64,
    },
    PayloadLengthMismatch {
        declared: usize,
        actual: usize,
    },
    InvalidUtf8(std::string::FromUtf8Error),
}

impl std::fmt::Display for ProtocolConvertError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyMessage => write!(formatter, "empty user message bytes"),
            Self::UnknownMessageType(code) => {
                write!(formatter, "unknown user message type: {code}")
            }
            Self::MessageTooShort {
                expected_at_least,
                actual,
            } => {
                write!(
                    formatter,
                    "message is too short: expected at least {expected_at_least} bytes, got {actual}"
                )
            }
            Self::PayloadTooLarge { length } => {
                write!(
                    formatter,
                    "payload is too large for u64 length: {length} bytes"
                )
            }
            Self::PayloadLengthMismatch { declared, actual } => {
                write!(
                    formatter,
                    "payload length mismatch: declared {declared} bytes, got {actual} bytes"
                )
            }
            Self::InvalidUtf8(error) => write!(formatter, "invalid UTF-8 payload: {error}"),
        }
    }
}

impl std::error::Error for ProtocolConvertError {}
