use super::{LogFile, Sink, Store, host_store};
use crate::external::{serde_json, uuid};
use crate::logging::store::HostStore;
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
    let store = Store::open_directory(&directory).expect("open host store");
    let host_sink = Sink::Host(HostStore::new(store).expect("start host writer"));
    assert!(host_store(Some(&host_sink)).is_ok());

    let file_sink =
        Sink::File(LogFile::create(&directory.join("marix.log")).expect("open fallback log"));
    assert!(matches!(
        host_store(Some(&file_sink)),
        Err(LoggingError::NotHosting)
    ));

    assert!(matches!(host_store(None), Err(LoggingError::NotHosting)));
}

#[test]
fn log_file_appends_json_lines_and_truncates_on_create() {
    let directory = temp_directory();
    let path = directory.join("marix.log");
    let mut file = LogFile::create(&path).expect("create fallback log");
    file.append(&message("first")).expect("append first");
    file.append(&message("second")).expect("append second");
    drop(file);

    let content = std::fs::read_to_string(&path).expect("read fallback log");
    let messages: Vec<LogMessage> = content
        .lines()
        .map(|line| serde_json::from_str(line).expect("deserialize JSON line"))
        .collect();
    let texts: Vec<&str> = messages
        .iter()
        .map(|entry| entry.message.as_str())
        .collect();
    assert_eq!(texts, vec!["first", "second"]);
    assert!(messages.iter().all(|entry| entry.arrival_ts == 0));

    let mut replaced = LogFile::create(&path).expect("truncate fallback log");
    replaced
        .append(&message("replacement"))
        .expect("append replacement");
    drop(replaced);

    let content = std::fs::read_to_string(&path).expect("read replaced log");
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 1);
    let replacement: LogMessage = serde_json::from_str(lines[0]).expect("deserialize replacement");
    assert_eq!(replacement.message, "replacement");
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
