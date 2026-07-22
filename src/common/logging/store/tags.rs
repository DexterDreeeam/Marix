use std::collections::BTreeSet;

use crate::external::redb::ReadableDatabase;
use crate::external::uuid;
use crate::logging::LoggingError;

use super::Store;
use super::schema;

impl Store {
    pub(super) fn tags(&self, session_id: Option<uuid::Uuid>) -> Result<Vec<String>, LoggingError> {
        let prefix = schema::session_key(session_id);
        let end = Self::lexicographic_successor(&prefix).ok_or_else(|| {
            LoggingError::Database("telemetry session tag key range overflow".to_owned())
        })?;
        let read = self
            .database
            .begin_read()
            .map_err(|error| LoggingError::Database(error.to_string()))?;
        let table = read
            .open_table(schema::SESSION_TAG_TABLE)
            .map_err(|error| LoggingError::Database(error.to_string()))?;
        let mut tags = BTreeSet::new();
        let mut range = table
            .range(prefix.as_slice()..end.as_slice())
            .map_err(|error| LoggingError::Database(error.to_string()))?;
        while let Some(entry) = range.next() {
            let (key, _value) = entry.map_err(|error| LoggingError::Database(error.to_string()))?;
            let tag_bytes = &key.value()[prefix.len()..];
            let tag = String::from_utf8(tag_bytes.to_vec())
                .map_err(|error| LoggingError::Database(error.to_string()))?;
            tags.insert(tag);
        }
        Ok(tags.into_iter().collect())
    }
}
