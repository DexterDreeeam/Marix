#[path = "store/reader.rs"]
mod reader;
#[path = "store/schema.rs"]
mod schema;
#[path = "store/tags.rs"]
mod tags;
#[path = "store/writer.rs"]
mod writer;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::config::Config;
use crate::external::redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
use crate::external::{serde_json, uuid};
use crate::logging::{LogMessage, LoggingError};

pub(super) use writer::HostStore;

const TELEMETRY_FILE_NAME: &str = "telemetry.redb";
const SESSION_KEY_LEN: usize = 17;
const SESSION_METADATA_LEN: usize = 32;
const TRIGRAM_COMPONENT_LEN: usize = 13;
const SESSION_RECORD_ID_KEY_LEN: usize = SESSION_KEY_LEN + 8;
const TRIGRAM_POSTING_KEY_LEN: usize = SESSION_KEY_LEN + TRIGRAM_COMPONENT_LEN + 16;
const SESSION_RECORD_ID_INDEX: TableDefinition<&[u8], u64> =
    TableDefinition::new("telemetry_session_record");

pub(super) struct Store {
    database: Database,
    next_id: AtomicU64,
    #[cfg(test)]
    batch_commits: AtomicU64,
}

impl Store {
    pub(super) fn open(config: &Config) -> Result<Self, LoggingError> {
        let directory = Self::database_directory(config);
        Self::open_directory(&directory)
    }

    pub(super) fn open_at(path: &Path) -> Result<Self, LoggingError> {
        let database =
            Database::create(path).map_err(|error| LoggingError::Database(error.to_string()))?;
        let store = Self {
            database,
            next_id: AtomicU64::new(0),
            #[cfg(test)]
            batch_commits: AtomicU64::new(0),
        };
        let next_id = store.ensure_schema()?;
        store.next_id.store(next_id, Ordering::Release);
        Ok(store)
    }

    #[cfg(test)]
    pub(super) fn record(&self, message: &LogMessage) -> Result<u64, LoggingError> {
        self.record_batch(std::slice::from_ref(message))
            .map(|ids| ids[0])
    }

    #[cfg(test)]
    pub(super) fn read_all(&self) -> Result<Vec<LogMessage>, LoggingError> {
        let read = self
            .database
            .begin_read()
            .map_err(|error| LoggingError::Database(error.to_string()))?;
        let table = read
            .open_table(schema::TELEMETRY_TABLE)
            .map_err(|error| LoggingError::Database(error.to_string()))?;
        let mut messages = Vec::new();
        for entry in table
            .iter()
            .map_err(|error| LoggingError::Database(error.to_string()))?
        {
            let (_id, value) = entry.map_err(|error| LoggingError::Database(error.to_string()))?;
            messages.push(
                serde_json::from_slice(value.value())
                    .map_err(|error| LoggingError::Serialization(error.to_string()))?,
            );
        }
        Ok(messages)
    }
}

// -- Private -- //

struct SessionMetadata {
    earliest_emit_ts: u64,
    latest_emit_ts: u64,
    count: u64,
    latest_record_id: u64,
}

impl SessionMetadata {
    fn new(emit_ts: u64, record_id: u64) -> Self {
        Self {
            earliest_emit_ts: emit_ts,
            latest_emit_ts: emit_ts,
            count: 1,
            latest_record_id: record_id,
        }
    }

    fn update(mut self, emit_ts: u64, record_id: u64) -> Result<Self, LoggingError> {
        self.earliest_emit_ts = self.earliest_emit_ts.min(emit_ts);
        self.latest_emit_ts = self.latest_emit_ts.max(emit_ts);
        self.count = self.count.checked_add(1).ok_or_else(|| {
            LoggingError::Database("telemetry session record count overflow".to_owned())
        })?;
        self.latest_record_id = self.latest_record_id.max(record_id);
        Ok(self)
    }

    fn encode(&self) -> [u8; SESSION_METADATA_LEN] {
        let mut encoded = [0; SESSION_METADATA_LEN];
        let values = [
            self.earliest_emit_ts,
            self.latest_emit_ts,
            self.count,
            self.latest_record_id,
        ];
        for (field, value) in encoded.chunks_exact_mut(8).zip(values) {
            field.copy_from_slice(&value.to_be_bytes());
        }
        encoded
    }

    fn decode(encoded: &[u8]) -> Result<Self, LoggingError> {
        if encoded.len() != SESSION_METADATA_LEN {
            return Err(LoggingError::Database(
                "invalid telemetry session metadata length".to_owned(),
            ));
        }
        let mut values = [0; 4];
        for (value, field) in values.iter_mut().zip(encoded.chunks_exact(8)) {
            let mut bytes = [0; 8];
            bytes.copy_from_slice(field);
            *value = u64::from_be_bytes(bytes);
        }
        let metadata = Self {
            earliest_emit_ts: values[0],
            latest_emit_ts: values[1],
            count: values[2],
            latest_record_id: values[3],
        };
        if metadata.count == 0 {
            return Err(LoggingError::Database(
                "telemetry session metadata count is zero".to_owned(),
            ));
        }
        if metadata.earliest_emit_ts > metadata.latest_emit_ts {
            return Err(LoggingError::Database(
                "telemetry session metadata timestamp range is invalid".to_owned(),
            ));
        }
        Ok(metadata)
    }
}

impl Store {
    fn session_record_id_key(
        session_id: Option<uuid::Uuid>,
        id: u64,
    ) -> [u8; SESSION_RECORD_ID_KEY_LEN] {
        let mut key = [0; SESSION_RECORD_ID_KEY_LEN];
        key[..SESSION_KEY_LEN].copy_from_slice(&schema::session_key(session_id));
        key[SESSION_KEY_LEN..].copy_from_slice(&id.to_be_bytes());
        key
    }

    fn trigram_posting_key(
        session_id: Option<uuid::Uuid>,
        component: &[u8; TRIGRAM_COMPONENT_LEN],
        emit_ts: u64,
        id: u64,
    ) -> [u8; TRIGRAM_POSTING_KEY_LEN] {
        let component_key = schema::trigram_key(session_id, component, id);
        let component_end = component_key.len() - 8;
        let mut key = [0; TRIGRAM_POSTING_KEY_LEN];
        key[..component_end].copy_from_slice(&component_key[..component_end]);
        key[component_end..component_end + 8].copy_from_slice(&(!emit_ts).to_be_bytes());
        key[component_end + 8..].copy_from_slice(&(!id).to_be_bytes());
        key
    }

    fn trigram_posting_position(key: &[u8]) -> Result<(u64, u64), LoggingError> {
        if key.len() != TRIGRAM_POSTING_KEY_LEN {
            return Err(LoggingError::Database(
                "invalid telemetry trigram posting key".to_owned(),
            ));
        }
        let emit_offset = TRIGRAM_POSTING_KEY_LEN - 16;
        let id_offset = TRIGRAM_POSTING_KEY_LEN - 8;
        let mut emit_bytes = [0; 8];
        emit_bytes.copy_from_slice(&key[emit_offset..id_offset]);
        let mut id_bytes = [0; 8];
        id_bytes.copy_from_slice(&key[id_offset..]);
        Ok((
            !u64::from_be_bytes(emit_bytes),
            !u64::from_be_bytes(id_bytes),
        ))
    }

    pub(super) fn open_directory(directory: &Path) -> Result<Self, LoggingError> {
        std::fs::create_dir_all(directory)?;
        let path = directory.join(TELEMETRY_FILE_NAME);
        if path.exists() {
            return Self::open_at(&path);
        }

        let legacy_paths = Self::legacy_database_paths(directory)?;
        if legacy_paths.is_empty() {
            return Self::open_at(&path);
        }
        Self::migrate_legacy(&path, &legacy_paths)
    }

    fn database_directory(config: &Config) -> PathBuf {
        let base = config
            .runtime
            .marix_path_server
            .as_deref()
            .unwrap_or(config.runtime.marix_path.as_str());
        PathBuf::from(base).join("log")
    }

    fn legacy_database_paths(directory: &Path) -> Result<Vec<PathBuf>, LoggingError> {
        let mut paths = Vec::new();
        for entry in std::fs::read_dir(directory)? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let file_name = entry.file_name();
            let Some(file_name) = file_name.to_str() else {
                continue;
            };
            if file_name.starts_with("telemetry-") && file_name.ends_with(".redb") {
                paths.push(entry.path());
            }
        }
        paths.sort();
        Ok(paths)
    }

    fn migrate_legacy(path: &Path, legacy_paths: &[PathBuf]) -> Result<Self, LoggingError> {
        let temporary_path = path.with_file_name(format!(
            "{TELEMETRY_FILE_NAME}.migrating-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4(),
        ));
        let migration = (|| {
            let migrated = Self::open_at(&temporary_path)?;
            for legacy_path in legacy_paths {
                let messages = Self::read_at(legacy_path)?;
                for batch in messages.chunks(writer::BATCH_SIZE) {
                    migrated.record_batch(batch)?;
                }
            }
            drop(migrated);
            std::fs::rename(&temporary_path, path)?;
            Self::open_at(path)
        })();

        match migration {
            Ok(store) => Ok(store),
            Err(error) => match std::fs::remove_file(&temporary_path) {
                Ok(()) => Err(error),
                Err(cleanup) if cleanup.kind() == std::io::ErrorKind::NotFound => Err(error),
                Err(cleanup) => Err(LoggingError::Io(format!(
                    "{error}; failed to remove migration database: {cleanup}",
                ))),
            },
        }
    }

    fn read_at(path: &Path) -> Result<Vec<LogMessage>, LoggingError> {
        let database =
            Database::open(path).map_err(|error| LoggingError::Database(error.to_string()))?;
        let read = database
            .begin_read()
            .map_err(|error| LoggingError::Database(error.to_string()))?;
        let table = read
            .open_table(schema::TELEMETRY_TABLE)
            .map_err(|error| LoggingError::Database(error.to_string()))?;
        let mut messages = Vec::new();
        for entry in table
            .iter()
            .map_err(|error| LoggingError::Database(error.to_string()))?
        {
            let (_id, value) = entry.map_err(|error| LoggingError::Database(error.to_string()))?;
            messages.push(
                serde_json::from_slice(value.value())
                    .map_err(|error| LoggingError::Serialization(error.to_string()))?,
            );
        }
        Ok(messages)
    }

    #[cfg(test)]
    fn batch_commit_count(&self) -> u64 {
        self.batch_commits.load(Ordering::Acquire)
    }
}

#[cfg(test)]
#[path = "store/tests.rs"]
mod tests;
