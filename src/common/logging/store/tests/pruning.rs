use std::collections::BTreeSet;

use crate::external::redb::{ReadableDatabase, ReadableTable, ReadableTableMetadata};
use crate::external::uuid;
use crate::logging::LogLevel;

use super::super::{SESSION_KEY_LEN, SESSION_RECORD_ID_INDEX, SessionMetadata, Store, schema};
use super::{message, query, temp_path};

fn primary_ids(store: &Store) -> BTreeSet<u64> {
    let read = store.database.begin_read().expect("read primary ids");
    let primary = read
        .open_table(schema::TELEMETRY_TABLE)
        .expect("open primary");
    primary
        .iter()
        .expect("iterate primary")
        .map(|entry| entry.expect("read primary entry").0.value())
        .collect()
}

fn assert_indexes_consistent(store: &Store, expected_records: u64) {
    let read = store.database.begin_read().expect("read indexes");
    let primary = read
        .open_table(schema::TELEMETRY_TABLE)
        .expect("open primary");
    assert_eq!(primary.len().expect("count primary"), expected_records);

    for definition in [
        schema::SESSION_TIME_INDEX,
        schema::SESSION_LEVEL_TIME_INDEX,
        SESSION_RECORD_ID_INDEX,
    ] {
        let index = read.open_table(definition).expect("open record index");
        assert_eq!(index.len().expect("count record index"), expected_records);
        for entry in index.iter().expect("iterate record index") {
            let (_key, id) = entry.expect("read record index entry");
            assert!(
                primary
                    .get(id.value())
                    .expect("look up indexed primary")
                    .is_some()
            );
        }
    }

    let trigrams = read
        .open_table(schema::TRIGRAM_INDEX)
        .expect("open trigram index");
    for entry in trigrams.iter().expect("iterate trigram index") {
        let (_key, id) = entry.expect("read trigram entry");
        assert!(
            primary
                .get(id.value())
                .expect("look up trigram primary")
                .is_some()
        );
    }

    let sessions = read
        .open_table(schema::SESSION_TABLE)
        .expect("open sessions");
    let mut metadata_records = 0_u64;
    for entry in sessions.iter().expect("iterate sessions") {
        let (_key, metadata) = entry.expect("read session entry");
        metadata_records += SessionMetadata::decode(metadata.value())
            .expect("decode session")
            .count;
    }
    assert_eq!(metadata_records, expected_records);

    let tags = read
        .open_table(schema::SESSION_TAG_TABLE)
        .expect("open session tags");
    for entry in tags.iter().expect("iterate session tags") {
        let (key, _value) = entry.expect("read session tag");
        assert!(key.value().len() >= SESSION_KEY_LEN);
        assert!(
            sessions
                .get(&key.value()[..SESSION_KEY_LEN])
                .expect("look up tagged session")
                .is_some()
        );
    }

    let metadata = read
        .open_table(schema::METADATA_TABLE)
        .expect("open metadata");
    assert_eq!(
        metadata
            .get(schema::META_INDEXED_COUNT)
            .expect("read indexed count")
            .expect("indexed count exists")
            .value(),
        expected_records
    );
}

#[test]
fn formal_sessions_prune_only_on_reopen_with_stable_ties() {
    let path = temp_path();
    let store = Store::open_at(&path).expect("open store");
    let mut sessions = Vec::new();
    let mut record_ids = Vec::new();
    for index in 0..52_u128 {
        let session = uuid::Uuid::from_u128(index + 1);
        let mut entry = message(
            Some(session),
            LogLevel::Info,
            100,
            format!("formal session {index}"),
        );
        entry.tags = vec!["formal".to_owned()];
        sessions.push(session);
        record_ids.push(store.record(&entry).expect("record formal session"));
    }
    let newest_pruned_id = store
        .record(&message(
            Some(sessions[0]),
            LogLevel::Error,
            200,
            "second record in oldest formal session",
        ))
        .expect("record newest id in oldest session");
    assert_eq!(store.sessions().expect("list runtime sessions").len(), 52);
    assert_eq!(store.read_all().expect("read runtime records").len(), 53);
    let historical_max = newest_pruned_id;
    drop(store);

    let reopened = Store::open_at(&path).expect("reopen and prune");
    assert_eq!(reopened.sessions().expect("list pruned sessions").len(), 50);
    let expected_ids: BTreeSet<u64> = record_ids[2..].iter().copied().collect();
    assert_eq!(primary_ids(&reopened), expected_ids);
    for id in &record_ids[..2] {
        assert!(
            reopened
                .record_by_id(*id)
                .expect("look up pruned record")
                .is_none()
        );
    }
    assert!(
        reopened
            .page(query(Some(sessions[0])))
            .expect("query pruned session")
            .items
            .is_empty()
    );
    assert!(
        reopened
            .record_by_id(newest_pruned_id)
            .expect("look up newest id from pruned session")
            .is_none()
    );
    let mut retained_query = query(Some(sessions[51]));
    retained_query.keyword = Some("formal session".to_owned());
    retained_query.tags = vec!["formal".to_owned()];
    let retained_page = reopened
        .page(retained_query)
        .expect("query retained session");
    assert_eq!(retained_page.items.len(), 1);
    assert_eq!(retained_page.items[0].id, record_ids[51]);
    assert_indexes_consistent(&reopened, 50);

    let next = reopened
        .record(&message(
            Some(sessions[51]),
            LogLevel::Info,
            101,
            "after formal prune",
        ))
        .expect("record after formal prune");
    assert!(next > historical_max);
}

#[test]
fn empty_records_keep_latest_time_and_id_set_only_on_reopen() {
    let path = temp_path();
    let store = Store::open_at(&path).expect("open store");
    let mut record_ids = Vec::new();
    for index in 0..1002_u64 {
        let emit_ts = match index {
            0 => 1_000_000,
            1 | 2 => 5,
            _ => 100 + index,
        };
        let mut entry = message(
            None,
            LogLevel::Info,
            emit_ts,
            format!("empty record {index}"),
        );
        entry.tags = vec!["empty".to_owned()];
        record_ids.push(store.record(&entry).expect("record Empty entry"));
    }
    assert_eq!(
        store.read_all().expect("read runtime Empty records").len(),
        1002
    );
    let historical_max = *record_ids.last().expect("historical maximum id");
    drop(store);

    let reopened = Store::open_at(&path).expect("reopen and prune Empty");
    let expected_ids: BTreeSet<u64> = std::iter::once(record_ids[0])
        .chain(record_ids[3..].iter().copied())
        .collect();
    assert_eq!(primary_ids(&reopened), expected_ids);
    assert_eq!(
        reopened
            .read_all()
            .expect("read pruned Empty records")
            .len(),
        1000
    );
    for id in [record_ids[1], record_ids[2]] {
        assert!(
            reopened
                .record_by_id(id)
                .expect("look up pruned Empty record")
                .is_none()
        );
    }
    assert!(
        reopened
            .record_by_id(record_ids[0])
            .expect("look up retained high timestamp")
            .is_some()
    );
    let page = reopened.page(query(None)).expect("query pruned Empty");
    assert_eq!(page.items[0].id, record_ids[0]);
    let empty_metadata = super::stored_session_metadata(&reopened, None);
    assert_eq!(empty_metadata.count, 1000);
    assert_indexes_consistent(&reopened, 1000);

    let next = reopened
        .record(&message(
            None,
            LogLevel::Info,
            1_000_001,
            "after Empty prune",
        ))
        .expect("record after Empty prune");
    assert!(next > historical_max);
}
