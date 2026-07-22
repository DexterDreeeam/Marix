use std::time::{SystemTime, UNIX_EPOCH};

use crate::external::*;
use crate::logging::{LogLevel, LogSource};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogMessage {
    #[serde(default)]
    pub source: LogSource,
    #[serde(alias = "tag")]
    pub level: LogLevel,
    pub message: String,
    pub session_id: Option<uuid::Uuid>,
    pub emit_ts: u64,
    pub arrival_ts: u64,
}

impl LogMessage {
    pub fn new(level: LogLevel, message: impl Into<String>) -> Self {
        Self {
            source: LogSource::default(),
            level,
            message: message.into(),
            session_id: None,
            emit_ts: Self::now_ms(),
            arrival_ts: 0,
        }
    }

    pub fn info(message: impl Into<String>) -> Self {
        Self::new(LogLevel::Info, message)
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(LogLevel::Warning, message)
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self::new(LogLevel::Error, message)
    }

    pub fn debug(message: impl Into<String>) -> Self {
        Self::new(LogLevel::Debug, message)
    }

    pub(crate) fn stamp_arrival(&mut self) {
        self.arrival_ts = Self::now_ms();
    }
}

#[cfg(test)]
mod tests {
    use crate::external::serde_json;
    use crate::logging::{LogLevel, LogMessage, LogSource};

    #[test]
    fn legacy_tag_deserializes_as_level_and_serializes_as_level() {
        let json = r#"{
            "tag":"Info",
            "message":"legacy",
            "session_id":null,
            "emit_ts":100,
            "arrival_ts":200
        }"#;

        let message: LogMessage = serde_json::from_str(json).expect("deserialize legacy log");

        assert_eq!(message.source, LogSource::Server);
        assert_eq!(message.level, LogLevel::Info);
        let serialized = serde_json::to_string(&message).expect("serialize current log");
        assert!(serialized.contains(r#""level":"Info""#));
        assert!(!serialized.contains(r#""tag":"#));
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
