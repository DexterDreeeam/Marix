use crate::external::uuid;
use crate::logging::LogLevel;

use super::super::{Store, schema};
use super::{message, query, temp_path};

#[test]
fn unicode_trigrams_are_session_isolated_and_fully_verified() {
    let store = Store::open_at(&temp_path()).expect("open store");
    let session = uuid::Uuid::new_v4();
    let other = uuid::Uuid::new_v4();
    let expected = store
        .record(&message(Some(session), LogLevel::Info, 100, "ÜBER Straße"))
        .expect("record Unicode match");
    store
        .record(&message(Some(session), LogLevel::Info, 90, "abc gap bcd"))
        .expect("record trigram false positive");
    store
        .record(&message(Some(other), LogLevel::Info, 110, "über straße"))
        .expect("record other session");

    let mut request = query(Some(session));
    request.keyword = Some("über STR".to_owned());
    let page = store.page(request).expect("Unicode trigram query");
    assert_eq!(
        page.items.iter().map(|item| item.id).collect::<Vec<_>>(),
        vec![expected]
    );

    let mut false_positive = query(Some(session));
    false_positive.keyword = Some("abcd".to_owned());
    assert!(
        store
            .page(false_positive)
            .expect("verified trigram query")
            .items
            .is_empty()
    );
}

#[test]
fn absent_long_keyword_uses_no_session_time_scan() {
    let store = Store::open_at(&temp_path()).expect("open store");
    let session = uuid::Uuid::new_v4();
    store
        .record(&message(
            Some(session),
            LogLevel::Info,
            10,
            "ordinary entry",
        ))
        .expect("record entry");
    let write = store.database.begin_write().expect("begin index removal");
    write
        .delete_table(schema::SESSION_TIME_INDEX)
        .expect("delete session time index");
    write.commit().expect("commit index removal");

    let mut request = query(Some(session));
    request.keyword = Some("zzqxv".to_owned());
    assert!(
        store
            .page(request)
            .expect("absent trigram query")
            .items
            .is_empty()
    );
}

#[test]
fn long_keyword_pages_verify_level_and_stable_time_id_cursors() {
    let store = Store::open_at(&temp_path()).expect("open store");
    let session = uuid::Uuid::new_v4();
    store
        .record(&message(
            Some(session),
            LogLevel::Info,
            500,
            "nee eed edl dle",
        ))
        .expect("record trigram false positive");
    let older = store
        .record(&message(Some(session), LogLevel::Info, 100, "needle older"))
        .expect("record older match");
    let tied_older = store
        .record(&message(Some(session), LogLevel::Info, 300, "needle tied"))
        .expect("record tied older match");
    let tied_newer = store
        .record(&message(
            Some(session),
            LogLevel::Info,
            300,
            "NEEDLE newest",
        ))
        .expect("record tied newer match");
    store
        .record(&message(
            Some(session),
            LogLevel::Debug,
            600,
            "needle below minimum level",
        ))
        .expect("record lower level");

    let mut request = query(Some(session));
    request.level = Some(LogLevel::Info);
    request.keyword = Some("Needle".to_owned());
    request.limit = 1;
    let first = store.page(request.clone()).expect("first keyword page");
    assert_eq!(first.items[0].id, tied_newer);
    request.before = first.next_cursor;
    let second = store.page(request.clone()).expect("second keyword page");
    assert_eq!(second.items[0].id, tied_older);
    request.before = second.next_cursor;
    let third = store.page(request).expect("third keyword page");
    assert_eq!(third.items[0].id, older);
    assert!(third.next_cursor.is_none());
}

#[test]
fn long_keyword_incremental_pages_advance_by_session_record_id() {
    let store = Store::open_at(&temp_path()).expect("open store");
    let session = uuid::Uuid::new_v4();
    let other = uuid::Uuid::new_v4();
    let base = store
        .record(&message(Some(session), LogLevel::Info, 1, "base"))
        .expect("record base");
    store
        .record(&message(Some(other), LogLevel::Info, 100, "needle other"))
        .expect("record other session");
    store
        .record(&message(
            Some(session),
            LogLevel::Debug,
            90,
            "needle below minimum",
        ))
        .expect("record lower level");
    let first = store
        .record(&message(Some(session), LogLevel::Info, 10, "NEEDLE first"))
        .expect("record first match");
    store
        .record(&message(Some(session), LogLevel::Info, 80, "not a match"))
        .expect("record sparse miss");
    let second = store
        .record(&message(Some(session), LogLevel::Info, 30, "needle second"))
        .expect("record second match");
    let third = store
        .record(&message(Some(session), LogLevel::Info, 20, "needle third"))
        .expect("record third match");
    let latest = store
        .record(&message(Some(other), LogLevel::Info, 200, "latest other"))
        .expect("record latest other session");

    let mut request = query(Some(session));
    request.level = Some(LogLevel::Info);
    request.keyword = Some("needle".to_owned());
    request.limit = 2;
    request.after_record_id = Some(base);
    let page = store.page(request.clone()).expect("first incremental page");
    assert_eq!(
        page.items.iter().map(|item| item.id).collect::<Vec<_>>(),
        vec![second, first]
    );
    assert_eq!(page.latest_record_id, Some(second));

    request.after_record_id = page.latest_record_id;
    let page = store.page(request).expect("second incremental page");
    assert_eq!(
        page.items.iter().map(|item| item.id).collect::<Vec<_>>(),
        vec![third]
    );
    assert_eq!(page.latest_record_id, Some(latest));
}

#[test]
fn short_keyword_uses_session_index_without_trigram_table() {
    let store = Store::open_at(&temp_path()).expect("open store");
    let session = uuid::Uuid::new_v4();
    let expected = store
        .record(&message(Some(session), LogLevel::Info, 10, "ÜB match"))
        .expect("record short match");
    let write = store.database.begin_write().expect("begin index removal");
    write
        .delete_table(schema::TRIGRAM_INDEX)
        .expect("delete trigram table");
    write.commit().expect("commit index removal");

    let mut request = query(Some(session));
    request.keyword = Some("üb".to_owned());
    let page = store.page(request).expect("short keyword query");
    assert_eq!(page.items[0].id, expected);
}
