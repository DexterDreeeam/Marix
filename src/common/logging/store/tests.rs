#[path = "tests/level.rs"]
mod level;
#[path = "tests/search.rs"]
mod search;

use std::sync::{Arc, Barrier};

use crate::external::redb::{Database, ReadableDatabase, ReadableTableMetadata, TableDefinition};
use crate::external::uuid;
use crate::logging::{LogLevel, LogMessage, LogPageQuery, LogSource, LoggingError};

use super::schema;
use super::{HostStore, SESSION_METADATA_LEN, SESSION_RECORD_ID_INDEX, SessionMetadata, Store};

fn temp_path() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "marix-telemetry-store-test-{}.redb",
        uuid::Uuid::new_v4()
    ))
}

fn message(
    session_id: Option<uuid::Uuid>,
    level: LogLevel,
    emit_ts: u64,
    text: impl Into<String>,
) -> LogMessage {
    let mut message = LogMessage::new(level, text);
    message.session_id = session_id;
    message.emit_ts = emit_ts;
    message
}

fn query(session_id: Option<uuid::Uuid>) -> LogPageQuery {
    LogPageQuery {
        session_id,
        limit: 200,
        ..LogPageQuery::default()
    }
}

fn stored_session_metadata(store: &Store, session_id: Option<uuid::Uuid>) -> SessionMetadata {
    let read = store.database.begin_read().expect("read session metadata");
    let table = read
        .open_table(schema::SESSION_TABLE)
        .expect("open session metadata");
    let metadata = table
        .get(schema::session_key(session_id).as_slice())
        .expect("read session metadata")
        .expect("session metadata exists");
    assert_eq!(metadata.value().len(), SESSION_METADATA_LEN);
    SessionMetadata::decode(metadata.value()).expect("decode session metadata")
}

#[test]
fn open_rebuilds_outdated_and_missing_indexes_without_changing_primary() {
    let path = temp_path();
    let session = uuid::Uuid::new_v4();
    let store = Store::open_at(&path).expect("open store");
    store
        .record(&message(Some(session), LogLevel::Info, 10, "first"))
        .expect("record first");
    store
        .record(&message(Some(session), LogLevel::Error, 20, "second"))
        .expect("record second");
    let before = store.read_all().expect("read primary before rebuild");
    let write = store.database.begin_write().expect("begin schema damage");
    write
        .delete_table(schema::TRIGRAM_INDEX)
        .expect("delete trigram index");
    {
        let mut metadata = write
            .open_table(schema::METADATA_TABLE)
            .expect("open metadata");
        metadata
            .insert(schema::META_SCHEMA_VERSION, schema::SCHEMA_VERSION - 1)
            .expect("downgrade version");
    }
    write.commit().expect("commit schema damage");
    drop(store);

    let rebuilt = Store::open_at(&path).expect("reopen and rebuild");
    assert_eq!(
        rebuilt.read_all().expect("read primary after rebuild"),
        before
    );
    let page = rebuilt
        .page(query(Some(session)))
        .expect("query rebuilt index");
    assert_eq!(
        page.items
            .iter()
            .map(|item| item.message_preview.as_str())
            .collect::<Vec<_>>(),
        vec!["second", "first"]
    );
    let read = rebuilt.database.begin_read().expect("read schema");
    let metadata = read
        .open_table(schema::METADATA_TABLE)
        .expect("open rebuilt metadata");
    assert_eq!(
        metadata
            .get(schema::META_SCHEMA_VERSION)
            .expect("read version")
            .expect("version exists")
            .value(),
        schema::SCHEMA_VERSION
    );
    let session_records = read
        .open_table(SESSION_RECORD_ID_INDEX)
        .expect("open rebuilt session record index");
    assert_eq!(session_records.len().expect("count session records"), 2);
}

#[test]
fn session_metadata_persists_all_fields_and_rebuilds_version_three() {
    let path = temp_path();
    let session = uuid::Uuid::new_v4();
    let store = Store::open_at(&path).expect("open store");
    store
        .record(&message(
            Some(session),
            LogLevel::Info,
            300,
            "identified latest",
        ))
        .expect("record identified latest timestamp");
    store
        .record(&message(None, LogLevel::Info, 80, "unknown middle"))
        .expect("record unknown middle timestamp");
    store
        .record(&message(
            Some(session),
            LogLevel::Info,
            100,
            "identified earliest",
        ))
        .expect("record identified earliest timestamp");
    store
        .record(&message(None, LogLevel::Info, 20, "unknown earliest"))
        .expect("record unknown earliest timestamp");
    let identified_latest_record = store
        .record(&message(
            Some(session),
            LogLevel::Info,
            200,
            "identified newest",
        ))
        .expect("record identified newest id");
    let unknown_latest_record = store
        .record(&message(None, LogLevel::Info, 90, "unknown newest"))
        .expect("record unknown newest id");

    let identified = stored_session_metadata(&store, Some(session));
    assert_eq!(identified.earliest_emit_ts, 100);
    assert_eq!(identified.latest_emit_ts, 300);
    assert_eq!(identified.count, 3);
    assert_eq!(identified.latest_record_id, identified_latest_record);
    let unknown = stored_session_metadata(&store, None);
    assert_eq!(unknown.earliest_emit_ts, 20);
    assert_eq!(unknown.latest_emit_ts, 90);
    assert_eq!(unknown.count, 3);
    assert_eq!(unknown.latest_record_id, unknown_latest_record);
    let sessions = store.sessions().expect("list sessions");
    assert_eq!(sessions[0].id, None);
    assert_eq!(sessions[0].emit_ts, 20);
    assert_eq!(sessions[1].id, Some(session));
    assert_eq!(sessions[1].emit_ts, 100);

    let before = store.read_all().expect("read primary before upgrade");
    let write = store.database.begin_write().expect("begin v3 conversion");
    write
        .delete_table(schema::SESSION_TABLE)
        .expect("delete v4 session metadata");
    {
        let legacy_sessions: TableDefinition<&[u8], u64> =
            TableDefinition::new("telemetry_sessions");
        let mut sessions = write
            .open_table(legacy_sessions)
            .expect("open v3 session metadata");
        sessions
            .insert(schema::session_key(Some(session)).as_slice(), 100)
            .expect("insert identified v3 metadata");
        sessions
            .insert(schema::session_key(None).as_slice(), 20)
            .expect("insert Unknown v3 metadata");
        let mut metadata = write
            .open_table(schema::METADATA_TABLE)
            .expect("open schema metadata");
        metadata
            .insert(schema::META_SCHEMA_VERSION, 3)
            .expect("set v3 schema");
    }
    write.commit().expect("commit v3 conversion");
    drop(store);

    let rebuilt = Store::open_at(&path).expect("rebuild v3 store");
    assert_eq!(rebuilt.read_all().expect("read rebuilt primary"), before);
    let read = rebuilt.database.begin_read().expect("read upgraded schema");
    let metadata = read
        .open_table(schema::METADATA_TABLE)
        .expect("open upgraded schema metadata");
    assert_eq!(
        metadata
            .get(schema::META_SCHEMA_VERSION)
            .expect("read upgraded version")
            .expect("upgraded version exists")
            .value(),
        schema::SCHEMA_VERSION
    );
    drop(metadata);
    drop(read);
    let identified = stored_session_metadata(&rebuilt, Some(session));
    assert_eq!(identified.earliest_emit_ts, 100);
    assert_eq!(identified.latest_emit_ts, 300);
    assert_eq!(identified.count, 3);
    assert_eq!(identified.latest_record_id, identified_latest_record);
    let unknown = stored_session_metadata(&rebuilt, None);
    assert_eq!(unknown.earliest_emit_ts, 20);
    assert_eq!(unknown.latest_emit_ts, 90);
    assert_eq!(unknown.count, 3);
    assert_eq!(unknown.latest_record_id, unknown_latest_record);
}

#[test]
fn invalid_session_metadata_is_rejected() {
    assert!(matches!(
        SessionMetadata::decode(&[0; SESSION_METADATA_LEN - 1]),
        Err(LoggingError::Database(_))
    ));
    let zero_count = SessionMetadata {
        earliest_emit_ts: 1,
        latest_emit_ts: 2,
        count: 0,
        latest_record_id: 3,
    };
    assert!(matches!(
        SessionMetadata::decode(&zero_count.encode()),
        Err(LoggingError::Database(_))
    ));
    let inverted_range = SessionMetadata {
        earliest_emit_ts: 2,
        latest_emit_ts: 1,
        count: 1,
        latest_record_id: 3,
    };
    assert!(matches!(
        SessionMetadata::decode(&inverted_range.encode()),
        Err(LoggingError::Database(_))
    ));
}

#[test]
fn failed_rebuild_rolls_back_partial_indexes_and_preserves_primary() {
    let path = temp_path();
    let session = uuid::Uuid::new_v4();
    let store = Store::open_at(&path).expect("open store");
    store
        .record(&message(Some(session), LogLevel::Info, 10, "valid"))
        .expect("record valid payload");
    let damaged = store
        .record(&message(Some(session), LogLevel::Info, 20, "damage"))
        .expect("record payload to damage");
    let write = store.database.begin_write().expect("begin damage");
    {
        let mut primary = write
            .open_table(schema::TELEMETRY_TABLE)
            .expect("open primary");
        primary
            .insert(damaged, b"{".as_slice())
            .expect("damage primary payload");
        let mut metadata = write
            .open_table(schema::METADATA_TABLE)
            .expect("open metadata");
        metadata
            .insert(schema::META_SCHEMA_VERSION, schema::SCHEMA_VERSION - 1)
            .expect("downgrade schema");
    }
    write.commit().expect("commit damage");
    drop(store);

    assert!(matches!(
        Store::open_at(&path),
        Err(LoggingError::Serialization(_))
    ));
    let database = Database::open(&path).expect("open database after failure");
    let read = database.begin_read().expect("read rolled back database");
    let time_index = read
        .open_table(schema::SESSION_TIME_INDEX)
        .expect("old time index remains");
    assert_eq!(time_index.len().expect("count old index"), 2);
    let primary = read
        .open_table(schema::TELEMETRY_TABLE)
        .expect("open preserved primary");
    assert_eq!(
        primary
            .get(damaged)
            .expect("read damaged payload")
            .expect("damaged payload exists")
            .value(),
        b"{"
    );
}

#[test]
fn session_and_level_pages_use_metadata_and_stable_cursors() {
    let store = Store::open_at(&temp_path()).expect("open store");
    let session = uuid::Uuid::new_v4();
    let other = uuid::Uuid::new_v4();
    let first = store
        .record(&message(Some(session), LogLevel::Info, 100, "first"))
        .expect("record first");
    let tied = store
        .record(&message(Some(session), LogLevel::Error, 100, "tied"))
        .expect("record tied");
    let newest = store
        .record(&message(Some(session), LogLevel::Error, 200, "newest"))
        .expect("record newest");
    store
        .record(&message(Some(other), LogLevel::Error, 300, "other"))
        .expect("record other");
    store
        .record(&message(None, LogLevel::Info, 50, "unknown"))
        .expect("record unknown");

    let sessions = store.sessions().expect("list sessions");
    assert_eq!(sessions[0].id, None);
    assert!(sessions.iter().any(|entry| entry.id == Some(session)));

    let mut request = query(Some(session));
    request.limit = 2;
    let page = store.page(request.clone()).expect("first page");
    assert_eq!(
        page.items.iter().map(|item| item.id).collect::<Vec<_>>(),
        vec![newest, tied]
    );
    request.before = page.next_cursor;
    let older = store.page(request).expect("older page");
    assert_eq!(
        older.items.iter().map(|item| item.id).collect::<Vec<_>>(),
        vec![first]
    );
    assert!(older.next_cursor.is_none());

    let mut leveled = query(Some(session));
    leveled.level = Some(LogLevel::Error);
    let leveled = store.page(leveled).expect("level page");
    assert_eq!(
        leveled.items.iter().map(|item| item.id).collect::<Vec<_>>(),
        vec![newest, tied]
    );
}

#[test]
fn equal_timestamp_cursor_orders_newer_record_id_first() {
    let store = Store::open_at(&temp_path()).expect("open store");
    let session = uuid::Uuid::new_v4();
    let older = store
        .record(&message(Some(session), LogLevel::Info, 100, "older id"))
        .expect("record older id");
    let newer = store
        .record(&message(Some(session), LogLevel::Info, 100, "newer id"))
        .expect("record newer id");
    let mut request = query(Some(session));
    request.limit = 1;
    let first = store.page(request.clone()).expect("first tied page");
    assert_eq!(first.items[0].id, newer);
    request.before = first.next_cursor;
    let second = store.page(request).expect("second tied page");
    assert_eq!(second.items[0].id, older);
}

#[test]
fn incremental_pages_advance_without_skipping_records() {
    let store = Store::open_at(&temp_path()).expect("open store");
    let session = uuid::Uuid::new_v4();
    let base = store
        .record(&message(Some(session), LogLevel::Info, 10, "base"))
        .expect("record base");
    let first = store
        .record(&message(Some(session), LogLevel::Info, 40, "first"))
        .expect("record first");
    let second = store
        .record(&message(Some(session), LogLevel::Info, 20, "second"))
        .expect("record second");
    let third = store
        .record(&message(Some(session), LogLevel::Info, 30, "third"))
        .expect("record third");

    let mut request = query(Some(session));
    request.limit = 1;
    request.after_record_id = Some(base);
    let page = store.page(request.clone()).expect("incremental page one");
    assert_eq!(page.items[0].id, first);
    assert_eq!(page.latest_record_id, Some(first));

    request.after_record_id = page.latest_record_id;
    let page = store.page(request.clone()).expect("incremental page two");
    assert_eq!(page.items[0].id, second);
    assert_eq!(page.latest_record_id, Some(second));

    request.after_record_id = page.latest_record_id;
    let page = store.page(request).expect("incremental page three");
    assert_eq!(page.items[0].id, third);
    assert_eq!(page.latest_record_id, Some(third));
}

#[test]
fn bounded_writer_batches_concurrent_records_and_flushes_reads() {
    const RECORDS: usize = 100;
    let host = HostStore::new(Store::open_at(&temp_path()).expect("open store"))
        .expect("start host writer");
    let session = uuid::Uuid::new_v4();
    let barrier = Arc::new(Barrier::new(RECORDS + 1));
    let mut workers = Vec::new();
    for index in 0..RECORDS {
        let host = host.clone();
        let barrier = Arc::clone(&barrier);
        workers.push(std::thread::spawn(move || {
            barrier.wait();
            host.record(message(
                Some(session),
                LogLevel::Info,
                index as u64,
                format!("record {index}"),
            ))
        }));
    }
    barrier.wait();
    for worker in workers {
        worker.join().expect("join writer").expect("record");
    }
    host.flush().expect("flush writer");
    assert!(
        host.batch_commit_count() < RECORDS as u64,
        "concurrent records were not batched"
    );
    assert_eq!(
        host.page(query(Some(session)))
            .expect("flush-aware page")
            .items
            .len(),
        RECORDS
    );
}

#[test]
fn full_record_lookup_returns_exact_message_and_legacy_source_defaults() {
    let path = temp_path();
    let database = Database::create(&path).expect("create legacy database");
    let write = database.begin_write().expect("begin legacy write");
    {
        let legacy_table: TableDefinition<u64, &[u8]> = schema::TELEMETRY_TABLE;
        let mut primary = write.open_table(legacy_table).expect("open primary");
        let legacy = br#"{
            "tag":"Warning",
            "message":"exact legacy payload",
            "session_id":null,
            "emit_ts":123,
            "arrival_ts":456
        }"#;
        primary.insert(0, legacy.as_slice()).expect("insert legacy");
    }
    write.commit().expect("commit legacy write");
    drop(database);

    let store = Store::open_at(&path).expect("open and index legacy store");
    let record = store
        .record_by_id(0)
        .expect("full record lookup")
        .expect("full record exists");
    assert_eq!(record.source, LogSource::Server);
    assert_eq!(record.level, LogLevel::Warning);
    assert_eq!(record.message, "exact legacy payload");
    assert_eq!(record.arrival_ts, 456);
    assert!(store.record_by_id(99).expect("missing lookup").is_none());
}

#[test]
fn invalid_cursor_and_conflicting_modes_are_rejected() {
    let store = Store::open_at(&temp_path()).expect("open store");
    let mut request = query(None);
    request.before = Some("not-a-cursor".to_owned());
    assert!(matches!(
        store.page(request),
        Err(LoggingError::InvalidQuery(_))
    ));

    let mut request = query(None);
    request.before = Some("0".repeat(32));
    request.after_record_id = Some(0);
    assert!(matches!(
        store.page(request),
        Err(LoggingError::InvalidQuery(_))
    ));
}
