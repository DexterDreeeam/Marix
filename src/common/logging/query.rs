use crate::external::uuid;
use crate::logging::logger::Logger;
use crate::logging::{LogMessage, LogTag, LoggingError};

impl Logger {
    /// Distinct session identifiers that appear anywhere in the local
    /// store. `None` (unassigned messages) is listed first when present,
    /// followed by every session ordered by that session's earliest
    /// `emit_ts`, newest first; ties are broken by the session's UUID
    /// string. Only available while this process is hosting a local store
    /// via [`Logger::host`].
    pub fn session_list() -> Result<Vec<Option<uuid::Uuid>>, LoggingError> {
        let messages = Self::local_log()?;
        Ok(distinct_sessions(&messages))
    }

    /// Every message recorded for one session (`None` selects unassigned
    /// messages), in ascending `emit_ts` order with ties broken by record
    /// (insertion) order. Only available while this process is hosting a
    /// local store via [`Logger::host`].
    pub fn session_log_list(
        session_id: Option<uuid::Uuid>,
    ) -> Result<Vec<LogMessage>, LoggingError> {
        let messages = Self::local_log()?;
        Ok(select_logs(messages, session_id, None, None))
    }

    /// Like [`Logger::session_log_list`], additionally narrowed by an exact
    /// tag match and/or a Unicode-aware, case-insensitive keyword substring
    /// match against the message text. A `None` or blank (after trimming)
    /// keyword applies no keyword filtering.
    pub fn session_log_filter(
        session_id: Option<uuid::Uuid>,
        tag: Option<LogTag>,
        keyword: Option<&str>,
    ) -> Result<Vec<LogMessage>, LoggingError> {
        let messages = Self::local_log()?;
        Ok(select_logs(messages, session_id, tag, keyword))
    }
}

// -- Private -- //

fn distinct_sessions(messages: &[LogMessage]) -> Vec<Option<uuid::Uuid>> {
    let mut has_unassigned = false;
    let mut earliest_by_session: std::collections::HashMap<uuid::Uuid, u64> =
        std::collections::HashMap::new();
    for message in messages {
        match message.session_id {
            None => has_unassigned = true,
            Some(id) => {
                earliest_by_session
                    .entry(id)
                    .and_modify(|earliest| *earliest = (*earliest).min(message.emit_ts))
                    .or_insert(message.emit_ts);
            }
        }
    }

    let mut sessions: Vec<(uuid::Uuid, u64)> = earliest_by_session.into_iter().collect();
    sessions.sort_by(|left, right| {
        right
            .1
            .cmp(&left.1)
            .then_with(|| left.0.to_string().cmp(&right.0.to_string()))
    });

    let mut result = Vec::with_capacity(sessions.len() + 1);
    if has_unassigned {
        result.push(None);
    }
    result.extend(sessions.into_iter().map(|(id, _earliest)| Some(id)));
    result
}

/// Filters `messages` down to one session, optionally narrowed by `tag`
/// and/or `keyword`, then returns them in ascending `emit_ts` order (ties
/// keep the original record-insertion order, since `messages` arrives in
/// that order from [`Logger::local_log`] and `sort_by_key` is stable).
fn select_logs(
    messages: Vec<LogMessage>,
    session_id: Option<uuid::Uuid>,
    tag: Option<LogTag>,
    keyword: Option<&str>,
) -> Vec<LogMessage> {
    let keyword = keyword
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_lowercase);

    let mut selected: Vec<LogMessage> = messages
        .into_iter()
        .filter(|message| message.session_id == session_id)
        .filter(|message| tag.is_none_or(|expected| expected == message.tag))
        .filter(|message| {
            keyword
                .as_ref()
                .is_none_or(|needle| message.message.to_lowercase().contains(needle.as_str()))
        })
        .collect();
    selected.sort_by_key(|message| message.emit_ts);
    selected
}

#[cfg(test)]
mod tests {
    use super::{distinct_sessions, select_logs};
    use crate::external::uuid;
    use crate::logging::logger::Store;
    use crate::logging::{LogMessage, LogTag};

    /// Opens a fresh, uniquely-named redb database under the OS temp
    /// directory. Each test gets its own file, so no test shares or
    /// pollutes any global/process state (unlike the `LOGGER` static).
    fn temp_store() -> Store {
        let path = std::env::temp_dir().join(format!(
            "marix-logging-query-test-{}.redb",
            uuid::Uuid::new_v4()
        ));
        Store::open_at(&path).expect("open temp store")
    }

    fn message(
        session_id: Option<uuid::Uuid>,
        tag: LogTag,
        emit_ts: u64,
        text: &str,
    ) -> LogMessage {
        let mut message = LogMessage::new(tag, text);
        message.session_id = session_id;
        message.emit_ts = emit_ts;
        message
    }

    #[test]
    fn session_list_reports_unassigned_first_when_present() {
        let store = temp_store();
        store
            .record(&message(None, LogTag::Info, 100, "boot"))
            .expect("record");

        let messages = store.read_all().expect("read_all");
        assert_eq!(distinct_sessions(&messages), vec![None]);
    }

    #[test]
    fn session_list_orders_two_uuids_newest_earliest_first() {
        let store = temp_store();
        let older = uuid::Uuid::new_v4();
        let newer = uuid::Uuid::new_v4();
        store
            .record(&message(Some(older), LogTag::Info, 100, "a"))
            .expect("record");
        store
            .record(&message(Some(newer), LogTag::Info, 200, "b"))
            .expect("record");
        store
            .record(&message(None, LogTag::Info, 50, "unassigned"))
            .expect("record");

        let messages = store.read_all().expect("read_all");
        assert_eq!(
            distinct_sessions(&messages),
            vec![None, Some(newer), Some(older)],
        );
    }

    #[test]
    fn session_list_breaks_earliest_ts_tie_by_uuid_string() {
        let store = temp_store();
        let first = uuid::Uuid::new_v4();
        let second = uuid::Uuid::new_v4();
        let (low, high) = if first.to_string() <= second.to_string() {
            (first, second)
        } else {
            (second, first)
        };
        store
            .record(&message(Some(high), LogTag::Info, 100, "high"))
            .expect("record");
        store
            .record(&message(Some(low), LogTag::Info, 100, "low"))
            .expect("record");

        let messages = store.read_all().expect("read_all");
        assert_eq!(distinct_sessions(&messages), vec![Some(low), Some(high)]);
    }

    #[test]
    fn session_log_list_sorts_by_emit_ts_then_record_order() {
        let store = temp_store();
        let session = uuid::Uuid::new_v4();
        store
            .record(&message(Some(session), LogTag::Info, 300, "third"))
            .expect("record");
        store
            .record(&message(Some(session), LogTag::Info, 100, "first-a"))
            .expect("record");
        store
            .record(&message(Some(session), LogTag::Warning, 100, "first-b"))
            .expect("record");
        store
            .record(&message(Some(session), LogTag::Info, 200, "second"))
            .expect("record");

        let messages = store.read_all().expect("read_all");
        let selected = select_logs(messages, Some(session), None, None);
        let texts: Vec<&str> = selected.iter().map(|m| m.message.as_str()).collect();
        // Ties at emit_ts 100 keep insertion order: "first-a" before
        // "first-b", since that is the order they were recorded in.
        assert_eq!(texts, vec!["first-a", "first-b", "second", "third"]);
    }

    #[test]
    fn session_log_filter_matches_exact_tag() {
        let store = temp_store();
        let session = uuid::Uuid::new_v4();
        store
            .record(&message(Some(session), LogTag::Info, 100, "info"))
            .expect("record");
        store
            .record(&message(Some(session), LogTag::Error, 200, "error"))
            .expect("record");

        let messages = store.read_all().expect("read_all");
        let selected = select_logs(messages, Some(session), Some(LogTag::Error), None);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].message, "error");
    }

    #[test]
    fn session_log_filter_keyword_is_unicode_case_insensitive() {
        let store = temp_store();
        let session = uuid::Uuid::new_v4();
        store
            .record(&message(Some(session), LogTag::Info, 100, "ÜBER cool"))
            .expect("record");
        store
            .record(&message(Some(session), LogTag::Info, 200, "unrelated"))
            .expect("record");

        let messages = store.read_all().expect("read_all");
        let selected = select_logs(messages, Some(session), None, Some("über"));
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].message, "ÜBER cool");
    }

    #[test]
    fn session_log_filter_blank_keyword_applies_no_filtering() {
        let store = temp_store();
        let session = uuid::Uuid::new_v4();
        store
            .record(&message(Some(session), LogTag::Info, 100, "a"))
            .expect("record");
        store
            .record(&message(Some(session), LogTag::Info, 200, "b"))
            .expect("record");

        let messages = store.read_all().expect("read_all");
        let selected = select_logs(messages, Some(session), None, Some("   "));
        assert_eq!(selected.len(), 2);
    }

    #[test]
    fn session_log_filter_no_match_returns_empty() {
        let store = temp_store();
        let session = uuid::Uuid::new_v4();
        store
            .record(&message(Some(session), LogTag::Info, 100, "hello"))
            .expect("record");

        let messages = store.read_all().expect("read_all");
        let by_tag = select_logs(messages.clone(), Some(session), Some(LogTag::Error), None);
        assert!(by_tag.is_empty());

        let by_keyword = select_logs(messages.clone(), Some(session), None, Some("nomatch"));
        assert!(by_keyword.is_empty());

        let other_session = uuid::Uuid::new_v4();
        let by_session = select_logs(messages, Some(other_session), None, None);
        assert!(by_session.is_empty());
    }
}
