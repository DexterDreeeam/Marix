use std::collections::{BTreeSet, HashSet};
use std::sync::atomic::Ordering;

use crate::external::redb::{
    ReadableDatabase, ReadableTable, ReadableTableMetadata, Table, TableDefinition, TableHandle,
};
use crate::external::{serde_json, uuid};
use crate::logging::{LogLevel, LogMessage, LoggingError};

use super::{
    SESSION_KEY_LEN, SESSION_RECORD_ID_INDEX, SessionMetadata, Store, TRIGRAM_COMPONENT_LEN,
};

pub(super) const TELEMETRY_TABLE: TableDefinition<u64, &[u8]> = TableDefinition::new("telemetry");
pub(super) const METADATA_TABLE: TableDefinition<&str, u64> =
    TableDefinition::new("telemetry_schema");
pub(super) const SESSION_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("telemetry_sessions");
pub(super) const SESSION_TIME_INDEX: TableDefinition<&[u8], u64> =
    TableDefinition::new("telemetry_session_emit");
pub(super) const SESSION_LEVEL_TIME_INDEX: TableDefinition<&[u8], u64> =
    TableDefinition::new("telemetry_session_level_emit");
pub(super) const TRIGRAM_INDEX: TableDefinition<&[u8], u64> =
    TableDefinition::new("telemetry_session_trigram");

pub(super) const SCHEMA_VERSION: u64 = 5;
pub(super) const META_SCHEMA_VERSION: &str = "schema_version";
pub(super) const META_INDEXED_COUNT: &str = "indexed_record_count";
pub(super) const META_NEXT_RECORD_ID: &str = "next_record_id";

const TIME_KEY_LEN: usize = SESSION_KEY_LEN + 16;
const LEVEL_TIME_KEY_LEN: usize = SESSION_KEY_LEN + 1 + 16;
const TRIGRAM_KEY_LEN: usize = SESSION_KEY_LEN + TRIGRAM_COMPONENT_LEN + 8;
const LEGACY_LEVEL_TIME_INDEX: TableDefinition<&[u8], u64> =
    TableDefinition::new("telemetry_session_tag_emit");

impl Store {
    pub(super) fn ensure_schema(&self) -> Result<u64, LoggingError> {
        let (rebuild, next_id) = {
            let read = self
                .database
                .begin_read()
                .map_err(|error| LoggingError::Database(error.to_string()))?;
            let names: HashSet<String> = read
                .list_tables()
                .map_err(|error| LoggingError::Database(error.to_string()))?
                .map(|table| table.name().to_owned())
                .collect();
            let (primary_count, expected_next_id) = if names.contains(TELEMETRY_TABLE.name()) {
                let primary = read
                    .open_table(TELEMETRY_TABLE)
                    .map_err(|error| LoggingError::Database(error.to_string()))?;
                let count = primary
                    .len()
                    .map_err(|error| LoggingError::Database(error.to_string()))?;
                let next_id = primary
                    .last()
                    .map_err(|error| LoggingError::Database(error.to_string()))?
                    .map(|(id, _value)| {
                        id.value().checked_add(1).ok_or_else(|| {
                            LoggingError::Database("telemetry record id overflow".to_owned())
                        })
                    })
                    .transpose()?
                    .unwrap_or(0);
                (count, next_id)
            } else {
                (0, 0)
            };
            let (version, indexed_count, next_id) = Self::metadata_values(&read, &names)?;
            let required = [
                TELEMETRY_TABLE.name(),
                METADATA_TABLE.name(),
                SESSION_TABLE.name(),
                SESSION_TIME_INDEX.name(),
                SESSION_LEVEL_TIME_INDEX.name(),
                SESSION_RECORD_ID_INDEX.name(),
                TRIGRAM_INDEX.name(),
            ];
            (
                version != Some(SCHEMA_VERSION)
                    || indexed_count != Some(primary_count)
                    || next_id != Some(expected_next_id)
                    || required.iter().any(|name| !names.contains(*name)),
                next_id,
            )
        };

        if rebuild {
            self.rebuild_indexes()
        } else {
            next_id.ok_or_else(|| {
                LoggingError::Database("telemetry next record id metadata is missing".to_owned())
            })
        }
    }

    pub(super) fn record_batch(&self, messages: &[LogMessage]) -> Result<Vec<u64>, LoggingError> {
        if messages.is_empty() {
            return Ok(Vec::new());
        }
        let serialized: Vec<Vec<u8>> = messages
            .iter()
            .map(|message| {
                serde_json::to_vec(message)
                    .map_err(|error| LoggingError::Serialization(error.to_string()))
            })
            .collect::<Result<_, _>>()?;
        let count = u64::try_from(messages.len())
            .map_err(|error| LoggingError::Database(error.to_string()))?;
        let first_id = self
            .next_id
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |next| {
                next.checked_add(count)
            })
            .map_err(|_| LoggingError::Database("telemetry record id overflow".to_owned()))?;
        let ids: Vec<u64> = (first_id..first_id + count).collect();

        let write = self
            .database
            .begin_write()
            .map_err(|error| LoggingError::Database(error.to_string()))?;
        {
            let mut primary = write
                .open_table(TELEMETRY_TABLE)
                .map_err(|error| LoggingError::Database(error.to_string()))?;
            let mut sessions = write
                .open_table(SESSION_TABLE)
                .map_err(|error| LoggingError::Database(error.to_string()))?;
            let mut session_time = write
                .open_table(SESSION_TIME_INDEX)
                .map_err(|error| LoggingError::Database(error.to_string()))?;
            let mut session_level_time = write
                .open_table(SESSION_LEVEL_TIME_INDEX)
                .map_err(|error| LoggingError::Database(error.to_string()))?;
            let mut session_record_id = write
                .open_table(SESSION_RECORD_ID_INDEX)
                .map_err(|error| LoggingError::Database(error.to_string()))?;
            let mut trigrams = write
                .open_table(TRIGRAM_INDEX)
                .map_err(|error| LoggingError::Database(error.to_string()))?;
            for ((id, message), bytes) in ids.iter().zip(messages).zip(serialized.iter()) {
                primary
                    .insert(*id, bytes.as_slice())
                    .map_err(|error| LoggingError::Database(error.to_string()))?;
                Self::index_message(
                    &mut sessions,
                    &mut session_time,
                    &mut session_level_time,
                    &mut session_record_id,
                    &mut trigrams,
                    *id,
                    message,
                )?;
            }
        }
        {
            let mut metadata = write
                .open_table(METADATA_TABLE)
                .map_err(|error| LoggingError::Database(error.to_string()))?;
            let previous_count = metadata
                .get(META_INDEXED_COUNT)
                .map_err(|error| LoggingError::Database(error.to_string()))?
                .map(|value| value.value())
                .unwrap_or(0);
            let previous_next = metadata
                .get(META_NEXT_RECORD_ID)
                .map_err(|error| LoggingError::Database(error.to_string()))?
                .map(|value| value.value())
                .unwrap_or(0);
            let indexed_count = previous_count.checked_add(count).ok_or_else(|| {
                LoggingError::Database("telemetry indexed record count overflow".to_owned())
            })?;
            metadata
                .insert(META_SCHEMA_VERSION, SCHEMA_VERSION)
                .map_err(|error| LoggingError::Database(error.to_string()))?;
            metadata
                .insert(META_INDEXED_COUNT, indexed_count)
                .map_err(|error| LoggingError::Database(error.to_string()))?;
            metadata
                .insert(META_NEXT_RECORD_ID, previous_next.max(first_id + count))
                .map_err(|error| LoggingError::Database(error.to_string()))?;
        }
        write
            .commit()
            .map_err(|error| LoggingError::Database(error.to_string()))?;
        #[cfg(test)]
        self.batch_commits.fetch_add(1, Ordering::AcqRel);
        Ok(ids)
    }
}

// -- Private -- //

impl Store {
    fn metadata_values(
        read: &crate::external::redb::ReadTransaction,
        names: &HashSet<String>,
    ) -> Result<(Option<u64>, Option<u64>, Option<u64>), LoggingError> {
        if !names.contains(METADATA_TABLE.name()) {
            return Ok((None, None, None));
        }
        let metadata = read
            .open_table(METADATA_TABLE)
            .map_err(|error| LoggingError::Database(error.to_string()))?;
        let version = metadata
            .get(META_SCHEMA_VERSION)
            .map_err(|error| LoggingError::Database(error.to_string()))?
            .map(|value| value.value());
        let count = metadata
            .get(META_INDEXED_COUNT)
            .map_err(|error| LoggingError::Database(error.to_string()))?
            .map(|value| value.value());
        let next_id = metadata
            .get(META_NEXT_RECORD_ID)
            .map_err(|error| LoggingError::Database(error.to_string()))?
            .map(|value| value.value());
        Ok((version, count, next_id))
    }

    fn rebuild_indexes(&self) -> Result<u64, LoggingError> {
        let write = self
            .database
            .begin_write()
            .map_err(|error| LoggingError::Database(error.to_string()))?;
        write
            .delete_table(SESSION_TABLE)
            .map_err(|error| LoggingError::Database(error.to_string()))?;
        for table in [
            SESSION_TIME_INDEX,
            SESSION_LEVEL_TIME_INDEX,
            LEGACY_LEVEL_TIME_INDEX,
            SESSION_RECORD_ID_INDEX,
            TRIGRAM_INDEX,
        ] {
            write
                .delete_table(table)
                .map_err(|error| LoggingError::Database(error.to_string()))?;
        }
        let mut indexed_count = 0_u64;
        let mut next_id = 0_u64;
        {
            let primary = write
                .open_table(TELEMETRY_TABLE)
                .map_err(|error| LoggingError::Database(error.to_string()))?;
            let mut sessions = write
                .open_table(SESSION_TABLE)
                .map_err(|error| LoggingError::Database(error.to_string()))?;
            let mut session_time = write
                .open_table(SESSION_TIME_INDEX)
                .map_err(|error| LoggingError::Database(error.to_string()))?;
            let mut session_level_time = write
                .open_table(SESSION_LEVEL_TIME_INDEX)
                .map_err(|error| LoggingError::Database(error.to_string()))?;
            let mut session_record_id = write
                .open_table(SESSION_RECORD_ID_INDEX)
                .map_err(|error| LoggingError::Database(error.to_string()))?;
            let mut trigrams = write
                .open_table(TRIGRAM_INDEX)
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
                    id,
                    &message,
                )?;
                indexed_count = indexed_count.checked_add(1).ok_or_else(|| {
                    LoggingError::Database("telemetry indexed record count overflow".to_owned())
                })?;
                next_id = id.checked_add(1).ok_or_else(|| {
                    LoggingError::Database("telemetry record id overflow".to_owned())
                })?;
            }
        }
        {
            let mut metadata = write
                .open_table(METADATA_TABLE)
                .map_err(|error| LoggingError::Database(error.to_string()))?;
            metadata
                .insert(META_SCHEMA_VERSION, SCHEMA_VERSION)
                .map_err(|error| LoggingError::Database(error.to_string()))?;
            metadata
                .insert(META_INDEXED_COUNT, indexed_count)
                .map_err(|error| LoggingError::Database(error.to_string()))?;
            metadata
                .insert(META_NEXT_RECORD_ID, next_id)
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
        id: u64,
        message: &LogMessage,
    ) -> Result<(), LoggingError> {
        let session = session_key(message.session_id);
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

        let time_key = session_time_key(message.session_id, message.emit_ts, id);
        session_time
            .insert(time_key.as_slice(), id)
            .map_err(|error| LoggingError::Database(error.to_string()))?;
        let level_key =
            session_level_time_key(message.session_id, message.level, message.emit_ts, id);
        session_level_time
            .insert(level_key.as_slice(), id)
            .map_err(|error| LoggingError::Database(error.to_string()))?;
        let record_key = Self::session_record_id_key(message.session_id, id);
        session_record_id
            .insert(record_key.as_slice(), id)
            .map_err(|error| LoggingError::Database(error.to_string()))?;
        for component in trigram_components(&message.message) {
            let trigram_key =
                Self::trigram_posting_key(message.session_id, &component, message.emit_ts, id);
            trigrams
                .insert(trigram_key.as_slice(), id)
                .map_err(|error| LoggingError::Database(error.to_string()))?;
        }
        Ok(())
    }
}

pub(super) fn session_key(session_id: Option<uuid::Uuid>) -> [u8; SESSION_KEY_LEN] {
    let mut key = [0; SESSION_KEY_LEN];
    if let Some(id) = session_id {
        key[0] = 1;
        key[1..].copy_from_slice(id.as_bytes());
    }
    key
}

pub(super) fn decode_session_key(key: &[u8]) -> Result<Option<uuid::Uuid>, LoggingError> {
    if key.len() != SESSION_KEY_LEN {
        return Err(LoggingError::Database(
            "invalid telemetry session metadata key".to_owned(),
        ));
    }
    match key[0] {
        0 if key[1..].iter().all(|byte| *byte == 0) => Ok(None),
        0 => Err(LoggingError::Database(
            "invalid telemetry Unknown session sentinel".to_owned(),
        )),
        1 => {
            let bytes: [u8; 16] = key[1..].try_into().map_err(|error| {
                LoggingError::Database(format!("invalid telemetry session UUID: {error}"))
            })?;
            Ok(Some(uuid::Uuid::from_bytes(bytes)))
        }
        _ => Err(LoggingError::Database(
            "invalid telemetry session sentinel".to_owned(),
        )),
    }
}

pub(super) fn session_time_key(
    session_id: Option<uuid::Uuid>,
    emit_ts: u64,
    id: u64,
) -> [u8; TIME_KEY_LEN] {
    let mut key = [0; TIME_KEY_LEN];
    key[..SESSION_KEY_LEN].copy_from_slice(&session_key(session_id));
    key[SESSION_KEY_LEN..SESSION_KEY_LEN + 8].copy_from_slice(&(!emit_ts).to_be_bytes());
    key[SESSION_KEY_LEN + 8..].copy_from_slice(&(!id).to_be_bytes());
    key
}

pub(super) fn session_level_time_key(
    session_id: Option<uuid::Uuid>,
    level: LogLevel,
    emit_ts: u64,
    id: u64,
) -> [u8; LEVEL_TIME_KEY_LEN] {
    let mut key = [0; LEVEL_TIME_KEY_LEN];
    key[..SESSION_KEY_LEN].copy_from_slice(&session_key(session_id));
    key[SESSION_KEY_LEN] = level_code(level);
    key[SESSION_KEY_LEN + 1..SESSION_KEY_LEN + 9].copy_from_slice(&(!emit_ts).to_be_bytes());
    key[SESSION_KEY_LEN + 9..].copy_from_slice(&(!id).to_be_bytes());
    key
}

pub(super) fn trigram_components(value: &str) -> Vec<[u8; TRIGRAM_COMPONENT_LEN]> {
    let characters: Vec<char> = value.to_lowercase().chars().collect();
    let mut unique = BTreeSet::new();
    for window in characters.windows(3) {
        let trigram: String = window.iter().collect();
        let bytes = trigram.as_bytes();
        let mut component = [0; TRIGRAM_COMPONENT_LEN];
        component[0] = bytes.len() as u8;
        component[1..1 + bytes.len()].copy_from_slice(bytes);
        unique.insert(component);
    }
    unique.into_iter().collect()
}

pub(super) fn trigram_key(
    session_id: Option<uuid::Uuid>,
    component: &[u8; TRIGRAM_COMPONENT_LEN],
    id: u64,
) -> [u8; TRIGRAM_KEY_LEN] {
    let mut key = [0; TRIGRAM_KEY_LEN];
    key[..SESSION_KEY_LEN].copy_from_slice(&session_key(session_id));
    key[SESSION_KEY_LEN..SESSION_KEY_LEN + TRIGRAM_COMPONENT_LEN].copy_from_slice(component);
    key[SESSION_KEY_LEN + TRIGRAM_COMPONENT_LEN..].copy_from_slice(&id.to_be_bytes());
    key
}

pub(super) fn prefix_bounds(prefix: &[u8], total_len: usize) -> (Vec<u8>, Vec<u8>) {
    let mut start = vec![0; total_len];
    let mut end = vec![u8::MAX; total_len];
    start[..prefix.len()].copy_from_slice(prefix);
    end[..prefix.len()].copy_from_slice(prefix);
    (start, end)
}

fn level_code(level: LogLevel) -> u8 {
    match level {
        LogLevel::Debug => 0,
        LogLevel::Info => 1,
        LogLevel::Warning => 2,
        LogLevel::Error => 3,
    }
}
