use crate::external::redb::{ReadTransaction, ReadableDatabase, ReadableTable};
use crate::logging::query::{log_record, validate_page_query};
use crate::logging::{
    LogLevel, LogMessage, LogPage, LogPageQuery, LogRecord, LogSession, LogSummary, LoggingError,
};

use super::schema;
use super::{SESSION_RECORD_ID_INDEX, SessionMetadata, Store, TRIGRAM_COMPONENT_LEN};

const MESSAGE_PREVIEW_CHARS: usize = 240;
const LOG_LEVELS: [LogLevel; 4] = [
    LogLevel::Debug,
    LogLevel::Info,
    LogLevel::Warning,
    LogLevel::Error,
];

struct Cursor {
    emit_ts: u64,
    id: u64,
}

struct StoredMessage {
    id: u64,
    message: LogMessage,
}

impl Store {
    pub(super) fn sessions(&self) -> Result<Vec<LogSession>, LoggingError> {
        let read = self.read_transaction()?;
        let table = read
            .open_table(schema::SESSION_TABLE)
            .map_err(|error| LoggingError::Database(error.to_string()))?;
        let mut unknown = None;
        let mut identified = Vec::new();
        for entry in table
            .iter()
            .map_err(|error| LoggingError::Database(error.to_string()))?
        {
            let (key, metadata) =
                entry.map_err(|error| LoggingError::Database(error.to_string()))?;
            let metadata = SessionMetadata::decode(metadata.value())?;
            let session = LogSession {
                id: schema::decode_session_key(key.value())?,
                emit_ts: metadata.earliest_emit_ts,
            };
            if session.id.is_none() {
                unknown = Some(session);
            } else {
                identified.push(session);
            }
        }
        identified.sort_by(|left, right| {
            right.emit_ts.cmp(&left.emit_ts).then_with(|| {
                left.id
                    .map(|id| id.to_string())
                    .cmp(&right.id.map(|id| id.to_string()))
            })
        });
        let mut sessions = Vec::with_capacity(identified.len() + usize::from(unknown.is_some()));
        sessions.extend(unknown);
        sessions.extend(identified);
        Ok(sessions)
    }

    pub(super) fn page(&self, query: LogPageQuery) -> Result<LogPage, LoggingError> {
        validate_page_query(&query)?;
        let cursor = query
            .before
            .as_deref()
            .map(Self::decode_cursor)
            .transpose()?;
        let keyword = query
            .keyword
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_lowercase);
        let trigram_components = keyword
            .as_deref()
            .filter(|value| value.chars().count() >= 3)
            .map(schema::trigram_components)
            .unwrap_or_default();
        let read = self.read_transaction()?;
        let latest = Self::latest_record_id(&read)?;

        if let Some(after_id) = query.after_record_id {
            return self.incremental_page(
                &read,
                &query,
                keyword.as_deref(),
                &trigram_components,
                after_id,
                latest,
            );
        }
        let records = if trigram_components.is_empty() {
            self.time_index_records(&read, &query, keyword.as_deref(), cursor.as_ref())?
        } else {
            self.trigram_index_records(
                &read,
                &query,
                keyword.as_deref(),
                &trigram_components,
                cursor.as_ref(),
            )?
        };
        Ok(Self::finish_page(records, query.limit, latest))
    }

    pub(super) fn record_by_id(&self, id: u64) -> Result<Option<LogRecord>, LoggingError> {
        let read = self.read_transaction()?;
        Self::message_by_id(&read, id).map(|message| message.map(|value| log_record(id, value)))
    }
}

// -- Private -- //

impl Store {
    fn read_transaction(&self) -> Result<ReadTransaction, LoggingError> {
        self.database
            .begin_read()
            .map_err(|error| LoggingError::Database(error.to_string()))
    }

    fn latest_record_id(read: &ReadTransaction) -> Result<Option<u64>, LoggingError> {
        let metadata = read
            .open_table(schema::METADATA_TABLE)
            .map_err(|error| LoggingError::Database(error.to_string()))?;
        let next_id = metadata
            .get(schema::META_NEXT_RECORD_ID)
            .map_err(|error| LoggingError::Database(error.to_string()))?
            .map(|value| value.value())
            .unwrap_or(0);
        Ok(next_id.checked_sub(1))
    }

    fn incremental_page(
        &self,
        read: &ReadTransaction,
        query: &LogPageQuery,
        keyword: Option<&str>,
        trigram_components: &[[u8; TRIGRAM_COMPONENT_LEN]],
        after_id: u64,
        latest: Option<u64>,
    ) -> Result<LogPage, LoggingError> {
        let mut records =
            self.session_incremental_records(read, query, keyword, trigram_components, after_id)?;
        let more = records.len() > query.limit;
        records.truncate(query.limit);
        let watermark = if more {
            records.last().map(|record| record.id)
        } else {
            latest
        };
        Self::sort_newest_first(&mut records);
        Ok(LogPage {
            items: records.into_iter().map(Self::summary).collect(),
            next_cursor: None,
            latest_record_id: watermark,
        })
    }

    fn session_incremental_records(
        &self,
        read: &ReadTransaction,
        query: &LogPageQuery,
        keyword: Option<&str>,
        trigram_components: &[[u8; TRIGRAM_COMPONENT_LEN]],
        after_id: u64,
    ) -> Result<Vec<StoredMessage>, LoggingError> {
        let Some(first_id) = after_id.checked_add(1) else {
            return Ok(Vec::new());
        };
        let start = Self::session_record_id_key(query.session_id, first_id);
        let end = Self::session_record_id_key(query.session_id, u64::MAX);
        let index = read
            .open_table(SESSION_RECORD_ID_INDEX)
            .map_err(|error| LoggingError::Database(error.to_string()))?;
        let primary = read
            .open_table(schema::TELEMETRY_TABLE)
            .map_err(|error| LoggingError::Database(error.to_string()))?;
        let trigrams = if trigram_components.is_empty() {
            None
        } else {
            Some(
                read.open_table(schema::TRIGRAM_INDEX)
                    .map_err(|error| LoggingError::Database(error.to_string()))?,
            )
        };
        let mut records = Vec::new();
        for entry in index
            .range(start.as_slice()..=end.as_slice())
            .map_err(|error| LoggingError::Database(error.to_string()))?
        {
            let (_key, id) = entry.map_err(|error| LoggingError::Database(error.to_string()))?;
            let id = id.value();
            let Some(message) = Self::message_from_table(&primary, id)? else {
                continue;
            };
            if let Some(trigrams) = trigrams.as_ref()
                && !Self::has_all_trigrams(
                    trigrams,
                    query,
                    message.emit_ts,
                    id,
                    trigram_components,
                )?
            {
                continue;
            }
            if Self::matches(&message, query, keyword) {
                records.push(StoredMessage { id, message });
                if records.len() > query.limit {
                    break;
                }
            }
        }
        Ok(records)
    }

    fn time_index_records(
        &self,
        read: &ReadTransaction,
        query: &LogPageQuery,
        keyword: Option<&str>,
        cursor: Option<&Cursor>,
    ) -> Result<Vec<StoredMessage>, LoggingError> {
        let primary = read
            .open_table(schema::TELEMETRY_TABLE)
            .map_err(|error| LoggingError::Database(error.to_string()))?;

        let mut records = Vec::new();
        if let Some(minimum) = query.level {
            let index = read
                .open_table(schema::SESSION_LEVEL_TIME_INDEX)
                .map_err(|error| LoggingError::Database(error.to_string()))?;
            for level in LOG_LEVELS.into_iter().filter(|level| *level >= minimum) {
                let key = schema::session_level_time_key(query.session_id, level, 0, 0);
                let prefix = &key[..key.len() - 16];
                let (start, end) = schema::prefix_bounds(prefix, key.len());
                let cursor_key = cursor.map(|value| {
                    schema::session_level_time_key(query.session_id, level, value.emit_ts, value.id)
                });
                records.extend(Self::index_range_records(
                    &index,
                    &primary,
                    start,
                    end,
                    cursor_key.as_ref().map(|key| key.as_slice()),
                    query,
                    keyword,
                )?);
            }
            return Ok(records);
        }

        let index = read
            .open_table(schema::SESSION_TIME_INDEX)
            .map_err(|error| LoggingError::Database(error.to_string()))?;
        let prefix = schema::session_key(query.session_id);
        let (start, end) = schema::prefix_bounds(&prefix, prefix.len() + 16);
        let cursor_key =
            cursor.map(|value| schema::session_time_key(query.session_id, value.emit_ts, value.id));
        Self::index_range_records(
            &index,
            &primary,
            start,
            end,
            cursor_key.as_ref().map(|key| key.as_slice()),
            query,
            keyword,
        )
    }

    fn trigram_index_records(
        &self,
        read: &ReadTransaction,
        query: &LogPageQuery,
        keyword: Option<&str>,
        components: &[[u8; TRIGRAM_COMPONENT_LEN]],
        cursor: Option<&Cursor>,
    ) -> Result<Vec<StoredMessage>, LoggingError> {
        let Some((driver, remaining)) = components.split_first() else {
            return Ok(Vec::new());
        };
        let driver_key = Self::trigram_posting_key(query.session_id, driver, 0, 0);
        let prefix_len = driver_key.len() - 16;
        let (start, end) = schema::prefix_bounds(&driver_key[..prefix_len], driver_key.len());
        let cursor_key = cursor.map(|value| {
            Self::trigram_posting_key(query.session_id, driver, value.emit_ts, value.id)
        });
        let range_start = cursor_key
            .as_ref()
            .and_then(|key| Self::lexicographic_successor(key.as_slice()))
            .unwrap_or(start);
        if range_start > end {
            return Ok(Vec::new());
        }

        let trigrams = read
            .open_table(schema::TRIGRAM_INDEX)
            .map_err(|error| LoggingError::Database(error.to_string()))?;
        let primary = read
            .open_table(schema::TELEMETRY_TABLE)
            .map_err(|error| LoggingError::Database(error.to_string()))?;
        let mut range = trigrams
            .range(range_start.as_slice()..=end.as_slice())
            .map_err(|error| LoggingError::Database(error.to_string()))?;
        let mut records = Vec::new();
        while let Some(entry) = range.next() {
            let (key, id) = entry.map_err(|error| LoggingError::Database(error.to_string()))?;
            let id = id.value();
            let (emit_ts, key_id) = Self::trigram_posting_position(key.value())?;
            if key_id != id {
                return Err(LoggingError::Database(
                    "telemetry trigram posting id mismatch".to_owned(),
                ));
            }
            if !Self::has_all_trigrams(&trigrams, query, emit_ts, id, remaining)? {
                continue;
            }
            if let Some(message) = Self::message_from_table(&primary, id)?
                && Self::matches(&message, query, keyword)
            {
                records.push(StoredMessage { id, message });
                if records.len() > query.limit {
                    break;
                }
            }
        }
        Ok(records)
    }

    fn has_all_trigrams(
        trigrams: &impl ReadableTable<&'static [u8], u64>,
        query: &LogPageQuery,
        emit_ts: u64,
        id: u64,
        components: &[[u8; TRIGRAM_COMPONENT_LEN]],
    ) -> Result<bool, LoggingError> {
        for component in components {
            let key = Self::trigram_posting_key(query.session_id, component, emit_ts, id);
            if trigrams
                .get(key.as_slice())
                .map_err(|error| LoggingError::Database(error.to_string()))?
                .is_none()
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn message_from_table(
        primary: &impl ReadableTable<u64, &'static [u8]>,
        id: u64,
    ) -> Result<Option<LogMessage>, LoggingError> {
        primary
            .get(id)
            .map_err(|error| LoggingError::Database(error.to_string()))?
            .map(|value| {
                crate::external::serde_json::from_slice(value.value())
                    .map_err(|error| LoggingError::Serialization(error.to_string()))
            })
            .transpose()
    }

    fn message_by_id(read: &ReadTransaction, id: u64) -> Result<Option<LogMessage>, LoggingError> {
        let primary = read
            .open_table(schema::TELEMETRY_TABLE)
            .map_err(|error| LoggingError::Database(error.to_string()))?;
        primary
            .get(id)
            .map_err(|error| LoggingError::Database(error.to_string()))?
            .map(|value| {
                crate::external::serde_json::from_slice(value.value())
                    .map_err(|error| LoggingError::Serialization(error.to_string()))
            })
            .transpose()
    }

    fn index_range_records(
        index: &impl ReadableTable<&'static [u8], u64>,
        primary: &impl ReadableTable<u64, &'static [u8]>,
        start: Vec<u8>,
        end: Vec<u8>,
        cursor_key: Option<&[u8]>,
        query: &LogPageQuery,
        keyword: Option<&str>,
    ) -> Result<Vec<StoredMessage>, LoggingError> {
        let range_start = cursor_key
            .and_then(Self::lexicographic_successor)
            .unwrap_or(start);
        if range_start > end {
            return Ok(Vec::new());
        }
        let mut range = index
            .range(range_start.as_slice()..=end.as_slice())
            .map_err(|error| LoggingError::Database(error.to_string()))?;
        let mut records = Vec::new();
        while let Some(entry) = range.next() {
            let (_key, id) = entry.map_err(|error| LoggingError::Database(error.to_string()))?;
            let id = id.value();
            if let Some(message) = Self::message_from_table(primary, id)?
                && Self::matches(&message, query, keyword)
            {
                records.push(StoredMessage { id, message });
                if records.len() > query.limit {
                    break;
                }
            }
        }
        Ok(records)
    }

    fn matches(message: &LogMessage, query: &LogPageQuery, keyword: Option<&str>) -> bool {
        message.session_id == query.session_id
            && query.level.is_none_or(|level| message.level >= level)
            && keyword.is_none_or(|value| message.message.to_lowercase().contains(value))
            && (query.tags.is_empty() || query.tags.iter().any(|tag| message.tags.contains(tag)))
    }

    fn finish_page(mut records: Vec<StoredMessage>, limit: usize, latest: Option<u64>) -> LogPage {
        Self::sort_newest_first(&mut records);
        let has_more = records.len() > limit;
        records.truncate(limit);
        let next_cursor = has_more
            .then(|| records.last().map(Self::encode_cursor))
            .flatten();
        LogPage {
            items: records.into_iter().map(Self::summary).collect(),
            next_cursor,
            latest_record_id: latest,
        }
    }

    fn sort_newest_first(records: &mut [StoredMessage]) {
        records.sort_by(|left, right| {
            right
                .message
                .emit_ts
                .cmp(&left.message.emit_ts)
                .then_with(|| right.id.cmp(&left.id))
        });
    }

    fn summary(record: StoredMessage) -> LogSummary {
        let message_len = record.message.message.chars().count();
        let message_preview: String = record
            .message
            .message
            .chars()
            .take(MESSAGE_PREVIEW_CHARS)
            .collect();
        LogSummary {
            id: record.id,
            source: record.message.source,
            level: record.message.level,
            session_id: record.message.session_id,
            emit_ts: record.message.emit_ts,
            message_preview,
            message_len,
            truncated: message_len > MESSAGE_PREVIEW_CHARS,
            tags: record.message.tags,
        }
    }

    fn encode_cursor(record: &StoredMessage) -> String {
        format!("{:016x}{:016x}", record.message.emit_ts, record.id)
    }

    fn decode_cursor(value: &str) -> Result<Cursor, LoggingError> {
        if value.len() != 32 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(LoggingError::InvalidQuery(
                "before cursor is invalid".to_owned(),
            ));
        }
        let emit_ts = u64::from_str_radix(&value[..16], 16)
            .map_err(|error| LoggingError::InvalidQuery(error.to_string()))?;
        let id = u64::from_str_radix(&value[16..], 16)
            .map_err(|error| LoggingError::InvalidQuery(error.to_string()))?;
        Ok(Cursor { emit_ts, id })
    }

    pub(super) fn lexicographic_successor(value: &[u8]) -> Option<Vec<u8>> {
        let mut next = value.to_vec();
        for byte in next.iter_mut().rev() {
            if *byte != u8::MAX {
                *byte += 1;
                return Some(next);
            }
            *byte = 0;
        }
        None
    }
}
