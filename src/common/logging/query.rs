use crate::external::*;
use crate::logging::{LogLevel, LogMessage, LogSession, LogSource, LoggingError};

const DEFAULT_PAGE_LIMIT: usize = 200;
const MAX_PAGE_LIMIT: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogPageQuery {
    pub session_id: Option<uuid::Uuid>,
    pub level: Option<LogLevel>,
    pub keyword: Option<String>,
    pub tags: Vec<String>,
    pub limit: usize,
    pub before: Option<String>,
    pub after_record_id: Option<u64>,
}

impl Default for LogPageQuery {
    fn default() -> Self {
        Self {
            session_id: None,
            level: None,
            keyword: None,
            tags: Vec::new(),
            limit: DEFAULT_PAGE_LIMIT,
            before: None,
            after_record_id: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogSummary {
    pub id: u64,
    pub source: LogSource,
    pub level: LogLevel,
    pub session_id: Option<uuid::Uuid>,
    pub emit_ts: u64,
    pub message_preview: String,
    pub message_len: usize,
    pub truncated: bool,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogPage {
    pub items: Vec<LogSummary>,
    pub next_cursor: Option<String>,
    pub latest_record_id: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogRecord {
    pub id: u64,
    pub source: LogSource,
    pub level: LogLevel,
    pub message: String,
    pub session_id: Option<uuid::Uuid>,
    pub emit_ts: u64,
    pub arrival_ts: u64,
    pub tags: Vec<String>,
}

impl Logger {
    pub fn session_list() -> Result<Vec<LogSession>, LoggingError> {
        Self::host_sink()?.sessions()
    }

    pub fn log_page(query: LogPageQuery) -> Result<LogPage, LoggingError> {
        Self::host_sink()?.page(query)
    }

    pub fn log_record(id: u64) -> Result<Option<LogRecord>, LoggingError> {
        Self::host_sink()?.record_by_id(id)
    }

    pub fn distinct_tags(session_id: Option<uuid::Uuid>) -> Result<Vec<String>, LoggingError> {
        Self::host_sink()?.tags(session_id)
    }
}

// -- Private -- //

use crate::logging::logger::Logger;

pub(super) fn validate_page_query(query: &LogPageQuery) -> Result<(), LoggingError> {
    if query.limit == 0 || query.limit > MAX_PAGE_LIMIT {
        return Err(LoggingError::InvalidQuery(format!(
            "limit must be between 1 and {MAX_PAGE_LIMIT}",
        )));
    }
    if query.before.is_some() && query.after_record_id.is_some() {
        return Err(LoggingError::InvalidQuery(
            "before and after_record_id are mutually exclusive".to_owned(),
        ));
    }
    Ok(())
}

pub(super) fn log_record(id: u64, message: LogMessage) -> LogRecord {
    LogRecord {
        id,
        source: message.source,
        level: message.level,
        message: message.message,
        session_id: message.session_id,
        emit_ts: message.emit_ts,
        arrival_ts: message.arrival_ts,
        tags: message.tags,
    }
}
