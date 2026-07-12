use super::{Sink, Store, host_store};
use crate::external::uuid;
use crate::logging::{LogMessage, LogTag, LoggingError};

fn temp_directory() -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "marix-logging-logger-test-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&path).expect("create temp directory");
    path
}

fn message(text: &str) -> LogMessage {
    LogMessage::new(LogTag::Info, text)
}

fn write_legacy(path: &std::path::Path, texts: &[&str]) {
    let store = Store::open_at(path).expect("open legacy store");
    for text in texts {
        store.record(&message(text)).expect("record legacy message");
    }
}

#[test]
fn host_store_permits_only_the_host_sink() {
    let directory = temp_directory();
    let host_sink = Sink::Host(Store::open_directory(&directory).expect("open host store"));
    assert!(host_store(Some(&host_sink)).is_ok());

    let local_directory = temp_directory();
    let local_sink =
        Sink::Local(Store::open_directory(&local_directory).expect("open local store"));
    assert!(matches!(
        host_store(Some(&local_sink)),
        Err(LoggingError::NotHosting)
    ));

    assert!(matches!(host_store(None), Err(LoggingError::NotHosting)));
}

#[test]
fn fixed_database_reopen_preserves_records() {
    let directory = temp_directory();
    let store = Store::open_directory(&directory).expect("open fixed store");
    store.record(&message("before restart")).expect("record");
    drop(store);

    let reopened = Store::open_directory(&directory).expect("reopen fixed store");
    let messages = reopened.read_all().expect("read fixed store");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].message, "before restart");
    assert!(directory.join("telemetry.redb").is_file());
}

#[test]
fn legacy_migration_aggregates_in_filename_order_and_keeps_files() {
    let directory = temp_directory();
    let first = directory.join("telemetry-20260712-100000-1.redb");
    let second = directory.join("telemetry-20260712-110000-2.redb");
    write_legacy(&second, &["second-a", "second-b"]);
    write_legacy(&first, &["first"]);

    let store = Store::open_directory(&directory).expect("migrate legacy stores");
    let messages = store.read_all().expect("read migrated store");
    let texts: Vec<&str> = messages
        .iter()
        .map(|entry| entry.message.as_str())
        .collect();
    assert_eq!(texts, vec!["first", "second-a", "second-b"]);
    assert!(first.is_file());
    assert!(second.is_file());
    assert!(directory.join("telemetry.redb").is_file());
}

#[test]
fn reopening_fixed_database_does_not_repeat_legacy_migration() {
    let directory = temp_directory();
    let legacy = directory.join("telemetry-20260712-100000-1.redb");
    write_legacy(&legacy, &["once"]);

    let migrated = Store::open_directory(&directory).expect("migrate legacy store");
    assert_eq!(migrated.read_all().expect("read migrated").len(), 1);
    drop(migrated);

    let reopened = Store::open_directory(&directory).expect("reopen fixed store");
    assert_eq!(reopened.read_all().expect("read reopened").len(), 1);
    assert!(legacy.is_file());
}
