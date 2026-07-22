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
    #[serde(default)]
    pub tags: Vec<String>,
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
            tags: Vec::new(),
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

// -- Private -- //

impl LogMessage {
    /// De-duplicates a message's own tags, preserving first-seen order.
    pub(super) fn dedup_tags(tags: Vec<String>) -> Vec<String> {
        let mut seen = std::collections::HashSet::with_capacity(tags.len());
        tags.into_iter()
            .filter(|tag| seen.insert(tag.clone()))
            .collect()
    }

    fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_millis() as u64)
            .unwrap_or(0)
    }
}
