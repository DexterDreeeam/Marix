use crate::external::uuid;
use crate::logging::LogLevel;

use super::super::Store;
use super::{message, query, temp_path};

#[test]
fn level_filter_uses_ordered_minimum_thresholds() {
    let store = Store::open_at(&temp_path()).expect("open store");
    let session = uuid::Uuid::new_v4();
    for (emit_ts, level) in [
        (10, LogLevel::Debug),
        (20, LogLevel::Info),
        (30, LogLevel::Warning),
        (40, LogLevel::Error),
    ] {
        store
            .record(&message(
                Some(session),
                level,
                emit_ts,
                format!("{level:?}"),
            ))
            .expect("record level fixture");
    }

    for (minimum, expected) in [
        (
            LogLevel::Debug,
            vec![
                LogLevel::Error,
                LogLevel::Warning,
                LogLevel::Info,
                LogLevel::Debug,
            ],
        ),
        (
            LogLevel::Info,
            vec![LogLevel::Error, LogLevel::Warning, LogLevel::Info],
        ),
        (LogLevel::Warning, vec![LogLevel::Error, LogLevel::Warning]),
        (LogLevel::Error, vec![LogLevel::Error]),
    ] {
        let mut request = query(Some(session));
        request.level = Some(minimum);
        let actual = store
            .page(request)
            .expect("query level threshold")
            .items
            .into_iter()
            .map(|item| item.level)
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }

    assert!(LogLevel::Debug < LogLevel::Info);
    assert!(LogLevel::Info < LogLevel::Warning);
    assert!(LogLevel::Warning < LogLevel::Error);
}
