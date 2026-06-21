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
