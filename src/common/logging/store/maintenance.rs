use std::collections::HashSet;

use crate::external::redb::{ReadableDatabase, ReadableTable, Table};
use crate::external::serde_json;
use crate::logging::{LogMessage, LoggingError};

use super::schema;
use super::{
    LEGACY_LEVEL_TIME_INDEX, SESSION_KEY_LEN, SESSION_RECORD_ID_INDEX, SessionMetadata, Store,
    StoreMaintenance,
};

const MAX_FORMAL_SESSIONS: usize = 50;
const MAX_EMPTY_RECORDS: usize = 1000;

impl StoreMaintenance for Store {
    fn prune_history(&self, next_id: u64) -> Result<u64, LoggingError> {
        let delete_ids = {
            let read = self
                .database
                .begin_read()
                .map_err(|error| LoggingError::Database(error.to_string()))?;
            let sessions = read
                .open_table(schema::SESSION_TABLE)
                .map_err(|error| LoggingError::Database(error.to_string()))?;
            let mut formal_sessions = Vec::new();
            let mut empty_count = 0_u64;
            for entry in sessions
                .iter()
                .map_err(|error| LoggingError::Database(error.to_string()))?
            {
                let (key, metadata) =
                    entry.map_err(|error| LoggingError::Database(error.to_string()))?;
                let key: [u8; SESSION_KEY_LEN] = key.value().try_into().map_err(|error| {
                    LoggingError::Database(format!(
                        "invalid telemetry session metadata key: {error}"
                    ))
                })?;
                let metadata = SessionMetadata::decode(metadata.value())?;
                if schema::decode_session_key(&key)?.is_some() {
                    formal_sessions.push((metadata.earliest_emit_ts, key));
                } else {
                    empty_count = metadata.count;
                }
            }
            if formal_sessions.len() <= MAX_FORMAL_SESSIONS
                && empty_count <= MAX_EMPTY_RECORDS as u64
            {
                return Ok(next_id);
            }

            formal_sessions.sort_unstable();
            let expired_session_count = formal_sessions.len().saturating_sub(MAX_FORMAL_SESSIONS);
            let expired_sessions: HashSet<[u8; SESSION_KEY_LEN]> = formal_sessions
                .into_iter()
                .take(expired_session_count)
                .map(|(_emit_ts, key)| key)
                .collect();

            let primary = read
                .open_table(schema::TELEMETRY_TABLE)
                .map_err(|error| LoggingError::Database(error.to_string()))?;
            let mut delete_ids = HashSet::new();
            let mut empty_records = Vec::new();
            for entry in primary
                .iter()
                .map_err(|error| LoggingError::Database(error.to_string()))?
            {
                let (id, value) =
                    entry.map_err(|error| LoggingError::Database(error.to_string()))?;
                let id = id.value();
                let message: LogMessage = serde_json::from_slice(value.value())
                    .map_err(|error| LoggingError::Serialization(error.to_string()))?;
                if expired_sessions.contains(&schema::session_key(message.session_id)) {
                    delete_ids.insert(id);
                } else if message.session_id.is_none() {
                    empty_records.push((message.emit_ts, id));
                }
            }
            empty_records.sort_unstable();
            let expired_empty_count = empty_records.len().saturating_sub(MAX_EMPTY_RECORDS);
            delete_ids.extend(
                empty_records
                    .into_iter()
                    .take(expired_empty_count)
                    .map(|(_emit_ts, id)| id),
            );
            delete_ids
        };

        if delete_ids.is_empty() {
            return Ok(next_id);
        }
        self.rebuild_indexes(next_id, &delete_ids)
    }

    fn rebuild_indexes(
        &self,
        next_id_floor: u64,
        delete_ids: &HashSet<u64>,
    ) -> Result<u64, LoggingError> {
        let write = self
            .database
            .begin_write()
            .map_err(|error| LoggingError::Database(error.to_string()))?;
        if !delete_ids.is_empty() {
            let mut primary = write
                .open_table(schema::TELEMETRY_TABLE)
                .map_err(|error| LoggingError::Database(error.to_string()))?;
            for id in delete_ids {
                primary
                    .remove(*id)
                    .map_err(|error| LoggingError::Database(error.to_string()))?;
            }
        }
        for table in [schema::SESSION_TABLE, schema::SESSION_TAG_TABLE] {
            write
                .delete_table(table)
                .map_err(|error| LoggingError::Database(error.to_string()))?;
        }
        for table in [
            schema::SESSION_TIME_INDEX,
            schema::SESSION_LEVEL_TIME_INDEX,
            LEGACY_LEVEL_TIME_INDEX,
            SESSION_RECORD_ID_INDEX,
            schema::TRIGRAM_INDEX,
        ] {
            write
                .delete_table(table)
                .map_err(|error| LoggingError::Database(error.to_string()))?;
        }

        let mut indexed_count = 0_u64;
        let mut next_id = next_id_floor;
        {
            let primary = write
                .open_table(schema::TELEMETRY_TABLE)
                .map_err(|error| LoggingError::Database(error.to_string()))?;
            let mut sessions = write
                .open_table(schema::SESSION_TABLE)
                .map_err(|error| LoggingError::Database(error.to_string()))?;
            let mut session_time = write
                .open_table(schema::SESSION_TIME_INDEX)
                .map_err(|error| LoggingError::Database(error.to_string()))?;
            let mut session_level_time = write
                .open_table(schema::SESSION_LEVEL_TIME_INDEX)
                .map_err(|error| LoggingError::Database(error.to_string()))?;
            let mut session_record_id = write
                .open_table(SESSION_RECORD_ID_INDEX)
                .map_err(|error| LoggingError::Database(error.to_string()))?;
            let mut trigrams = write
                .open_table(schema::TRIGRAM_INDEX)
                .map_err(|error| LoggingError::Database(error.to_string()))?;
            let mut session_tags = write
                .open_table(schema::SESSION_TAG_TABLE)
                .map_err(|error| LoggingError::Database(error.to_string()))?;
            for entry in primary
                .iter()
                .map_err(|error| LoggingError::Database(error.to_string()))?
            {
                let (id, value) =
                    entry.map_err(|error| LoggingError::Database(error.to_string()))?;
                let id = id.value();
                let message: LogMessage = serde_json::from_slice(value.value())
                    .map_err(|error| LoggingError::Serialization(error.to_string()))?;
                Self::index_message(
                    &mut sessions,
                    &mut session_time,
                    &mut session_level_time,
                    &mut session_record_id,
                    &mut trigrams,
                    &mut session_tags,
                    id,
                    &message,
                )?;
                indexed_count = indexed_count.checked_add(1).ok_or_else(|| {
                    LoggingError::Database("telemetry indexed record count overflow".to_owned())
                })?;
                let record_next = id.checked_add(1).ok_or_else(|| {
                    LoggingError::Database("telemetry record id overflow".to_owned())
                })?;
                next_id = next_id.max(record_next);
            }
        }
        {
            let mut metadata = write
                .open_table(schema::METADATA_TABLE)
                .map_err(|error| LoggingError::Database(error.to_string()))?;
            metadata
                .insert(schema::META_SCHEMA_VERSION, schema::SCHEMA_VERSION)
                .map_err(|error| LoggingError::Database(error.to_string()))?;
            metadata
                .insert(schema::META_INDEXED_COUNT, indexed_count)
                .map_err(|error| LoggingError::Database(error.to_string()))?;
            metadata
                .insert(schema::META_NEXT_RECORD_ID, next_id)
                .map_err(|error| LoggingError::Database(error.to_string()))?;
        }
        write
            .commit()
            .map_err(|error| LoggingError::Database(error.to_string()))?;
        Ok(next_id)
    }

    fn index_message(
        sessions: &mut Table<'_, &[u8], &[u8]>,
        session_time: &mut Table<'_, &[u8], u64>,
        session_level_time: &mut Table<'_, &[u8], u64>,
        session_record_id: &mut Table<'_, &[u8], u64>,
        trigrams: &mut Table<'_, &[u8], u64>,
        session_tags: &mut Table<'_, &[u8], &[u8]>,
        id: u64,
        message: &LogMessage,
    ) -> Result<(), LoggingError> {
        let session = schema::session_key(message.session_id);
        let metadata = sessions
            .get(session.as_slice())
            .map_err(|error| LoggingError::Database(error.to_string()))?
            .map(|value| SessionMetadata::decode(value.value()))
            .transpose()?
            .map(|metadata| metadata.update(message.emit_ts, id))
            .transpose()?
            .unwrap_or_else(|| SessionMetadata::new(message.emit_ts, id));
        let encoded = metadata.encode();
        sessions
            .insert(session.as_slice(), encoded.as_slice())
            .map_err(|error| LoggingError::Database(error.to_string()))?;

        let time_key = schema::session_time_key(message.session_id, message.emit_ts, id);
        session_time
            .insert(time_key.as_slice(), id)
            .map_err(|error| LoggingError::Database(error.to_string()))?;
        let level_key =
            schema::session_level_time_key(message.session_id, message.level, message.emit_ts, id);
        session_level_time
            .insert(level_key.as_slice(), id)
            .map_err(|error| LoggingError::Database(error.to_string()))?;
        let record_key = Self::session_record_id_key(message.session_id, id);
        session_record_id
            .insert(record_key.as_slice(), id)
            .map_err(|error| LoggingError::Database(error.to_string()))?;
        for component in schema::trigram_components(&message.message) {
            let trigram_key =
                Self::trigram_posting_key(message.session_id, &component, message.emit_ts, id);
            trigrams
                .insert(trigram_key.as_slice(), id)
                .map_err(|error| LoggingError::Database(error.to_string()))?;
        }
        let empty_value: &[u8] = b"";
        for tag in &message.tags {
            let tag_key = schema::session_tag_key(message.session_id, tag);
            session_tags
                .insert(tag_key.as_slice(), empty_value)
                .map_err(|error| LoggingError::Database(error.to_string()))?;
        }
        Ok(())
    }
}
