use std::time::{SystemTime, UNIX_EPOCH};

use crate::external::*;
use crate::logging::LogTag;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogMessage {
    pub tag: LogTag,
    pub message: String,
    pub session_id: Option<uuid::Uuid>,
    pub emit_ts: u64,
    pub arrival_ts: u64,
}

impl LogMessage {
    pub fn new(tag: LogTag, message: impl Into<String>) -> Self {
        Self {
            tag,
            message: message.into(),
            session_id: None,
            emit_ts: Self::now_ms(),
            arrival_ts: 0,
        }
    }

    pub fn info(message: impl Into<String>) -> Self {
        Self::new(LogTag::Info, message)
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(LogTag::Warning, message)
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self::new(LogTag::Error, message)
    }

    pub fn debug(message: impl Into<String>) -> Self {
        Self::new(LogTag::Debug, message)
    }

    pub(crate) fn stamp_arrival(&mut self) {
        self.arrival_ts = Self::now_ms();
    }
}

// -- Private -- //

impl LogMessage {
    fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_millis() as u64)
            .unwrap_or(0)
    }
}
