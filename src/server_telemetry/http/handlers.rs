use axum::response::IntoResponse;
use marix_common::{LogTag, Logger, LoggingError};

use crate::http::external::*;

const PAGE_HTML: &str = include_str!("page.html");

/// Serves the single-file telemetry page.
pub(super) async fn root() -> axum::response::Response {
    (
        axum::http::StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
        PAGE_HTML,
    )
        .into_response()
}

/// `GET /api/sessions`: every distinct session and its earliest emit time.
/// The unknown session is first when present; identified sessions follow
/// from newest to oldest earliest emit time.
pub(super) async fn sessions() -> axum::response::Response {
    match Logger::session_list() {
        Ok(sessions) => axum::Json(sessions).into_response(),
        Err(error) => query_error_response(error),
    }
}

#[derive(Deserialize)]
pub(super) struct LogsQuery {
    session_id: Option<String>,
    tag: Option<String>,
    keyword: Option<String>,
}

/// `GET /api/logs`: `session_id` is required (a UUID string or `unknown`;
/// legacy `unassigned` is also accepted). `tag` and `keyword` are optional
/// narrowing filters. A missing or blank `tag` means "all tags"; a non-blank
/// `tag` is parsed case-insensitively and rejected with `400` if invalid.
pub(super) async fn logs(
    axum::extract::Query(query): axum::extract::Query<LogsQuery>,
) -> axum::response::Response {
    let raw_session_id = match query.session_id {
        Some(value) => value,
        None => return bad_request("missing session_id"),
    };
    let session_id = match parse_session_id(&raw_session_id) {
        Ok(session_id) => session_id,
        Err(message) => return bad_request(message),
    };
    let tag = match query.tag.as_deref().map(str::trim) {
        Some(trimmed) if !trimmed.is_empty() => match parse_tag(trimmed) {
            Ok(tag) => Some(tag),
            Err(message) => return bad_request(message),
        },
        _ => None,
    };
    let keyword = query
        .keyword
        .as_deref()
        .filter(|value| !value.trim().is_empty());

    let result = if tag.is_none() && keyword.is_none() {
        Logger::session_log_list(session_id)
    } else {
        Logger::session_log_filter(session_id, tag, keyword)
    };
    match result {
        Ok(messages) => axum::Json(messages).into_response(),
        Err(error) => query_error_response(error),
    }
}

// -- Private -- //

fn parse_session_id(raw: &str) -> Result<Option<uuid::Uuid>, &'static str> {
    if raw == "unknown" || raw == "unassigned" {
        return Ok(None);
    }
    uuid::Uuid::parse_str(raw)
        .map(Some)
        .map_err(|_error| "invalid session_id")
}

fn parse_tag(raw: &str) -> Result<LogTag, &'static str> {
    match raw.to_lowercase().as_str() {
        "info" => Ok(LogTag::Info),
        "warning" => Ok(LogTag::Warning),
        "error" => Ok(LogTag::Error),
        "debug" => Ok(LogTag::Debug),
        _ => Err("invalid tag"),
    }
}

fn bad_request(message: &str) -> axum::response::Response {
    (
        axum::http::StatusCode::BAD_REQUEST,
        axum::Json(serde_json::json!({ "error": message })),
    )
        .into_response()
}

/// Maps any local-store query failure to a generic `500`, logging the real
/// cause (which may include file paths) only to the local telemetry log,
/// never in the HTTP response body.
fn query_error_response(error: LoggingError) -> axum::response::Response {
    Logger::error(format!("telemetry query failed: {error}"));
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        axum::Json(serde_json::json!({ "error": "internal server error" })),
    )
        .into_response()
}
