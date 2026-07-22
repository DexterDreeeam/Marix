use axum::response::IntoResponse;
use marix_common::{LogLevel, LogPageQuery, Logger, LoggingError};

use crate::external::*;

const PAGE_HTML: &str = include_str!("http/page.html");
const PAGE_CSS: &str = include_str!("http/page.css");
const DATA_SCRIPT: &str = include_str!("http/telemetry-data.js");
const DROPDOWN_SCRIPT: &str = include_str!("http/telemetry-dropdown.js");
const FORMAT_SCRIPT: &str = include_str!("http/telemetry-format.js");
const MESSAGE_SCRIPT: &str = include_str!("http/telemetry-message.js");
const PAGE_SCRIPT: &str = include_str!("http/telemetry.js");
const FAVICON_SVG: &str = include_str!("http/favicon.svg");
const DEFAULT_LIMIT: usize = 200;
const MAX_LIMIT: usize = 500;

pub(super) async fn root() -> axum::response::Response {
    static_response("text/html; charset=utf-8", PAGE_HTML)
}

pub(super) async fn favicon() -> axum::response::Response {
    static_response("image/svg+xml", FAVICON_SVG)
}

pub(super) async fn stylesheet() -> axum::response::Response {
    static_response("text/css; charset=utf-8", PAGE_CSS)
}

pub(super) async fn data_script() -> axum::response::Response {
    static_response("text/javascript; charset=utf-8", DATA_SCRIPT)
}

pub(super) async fn dropdown_script() -> axum::response::Response {
    static_response("text/javascript; charset=utf-8", DROPDOWN_SCRIPT)
}

pub(super) async fn format_script() -> axum::response::Response {
    static_response("text/javascript; charset=utf-8", FORMAT_SCRIPT)
}

pub(super) async fn message_script() -> axum::response::Response {
    static_response("text/javascript; charset=utf-8", MESSAGE_SCRIPT)
}

pub(super) async fn script() -> axum::response::Response {
    static_response("text/javascript; charset=utf-8", PAGE_SCRIPT)
}

pub(super) async fn sessions() -> axum::response::Response {
    match tokio::task::spawn_blocking(Logger::session_list).await {
        Ok(Ok(sessions)) => axum::Json(sessions).into_response(),
        Ok(Err(error)) => query_error_response(error),
        Err(error) => blocking_error_response(error),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LogsQuery {
    session_id: Option<String>,
    level: Option<String>,
    keyword: Option<String>,
    tags: Option<String>,
    limit: Option<usize>,
    before: Option<String>,
    after_id: Option<u64>,
}

pub(super) async fn logs(
    axum::extract::Query(query): axum::extract::Query<LogsQuery>,
) -> axum::response::Response {
    let raw_session_id = match query.session_id.as_deref() {
        Some(value) => value,
        None => return bad_request("missing session_id"),
    };
    let session_id = match parse_session_id(raw_session_id) {
        Ok(session_id) => session_id,
        Err(message) => return bad_request(message),
    };
    let level = match query.level.as_deref().map(str::trim) {
        Some(value) if !value.is_empty() => match parse_level(value) {
            Ok(level) => Some(level),
            Err(message) => return bad_request(message),
        },
        _ => None,
    };
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT);
    if limit == 0 || limit > MAX_LIMIT {
        return bad_request("limit must be between 1 and 500");
    }
    if query.before.is_some() && query.after_id.is_some() {
        return bad_request("before and after_id are mutually exclusive");
    }
    let tags: Vec<String> = query
        .tags
        .as_deref()
        .unwrap_or("")
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect();
    let request = LogPageQuery {
        session_id,
        level,
        keyword: query
            .keyword
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty()),
        tags,
        limit,
        before: query.before,
        after_record_id: query.after_id,
    };

    match tokio::task::spawn_blocking(move || Logger::log_page(request)).await {
        Ok(Ok(page)) => axum::Json(page).into_response(),
        Ok(Err(error)) => query_error_response(error),
        Err(error) => blocking_error_response(error),
    }
}

pub(super) async fn log_record(
    axum::extract::Path(id): axum::extract::Path<u64>,
) -> axum::response::Response {
    match tokio::task::spawn_blocking(move || Logger::log_record(id)).await {
        Ok(Ok(Some(record))) => axum::Json(record).into_response(),
        Ok(Ok(None)) => (
            axum::http::StatusCode::NOT_FOUND,
            axum::Json(serde_json::json!({ "error": "log record not found" })),
        )
            .into_response(),
        Ok(Err(error)) => query_error_response(error),
        Err(error) => blocking_error_response(error),
    }
}

pub(super) async fn session_tags(
    axum::extract::Path(raw_session_id): axum::extract::Path<String>,
) -> axum::response::Response {
    let session_id = match parse_session_id(&raw_session_id) {
        Ok(session_id) => session_id,
        Err(message) => return bad_request(message),
    };
    match tokio::task::spawn_blocking(move || Logger::distinct_tags(session_id)).await {
        Ok(Ok(tags)) => axum::Json(tags).into_response(),
        Ok(Err(error)) => query_error_response(error),
        Err(error) => blocking_error_response(error),
    }
}

// -- Private -- //

fn static_response(content_type: &'static str, body: &'static str) -> axum::response::Response {
    (
        axum::http::StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, content_type)],
        body,
    )
        .into_response()
}

fn parse_session_id(raw: &str) -> Result<Option<uuid::Uuid>, &'static str> {
    let raw = raw.trim();
    if raw == "unknown" || raw == "unassigned" {
        return Ok(None);
    }
    uuid::Uuid::parse_str(raw)
        .map(Some)
        .map_err(|_error| "invalid session_id")
}

fn parse_level(raw: &str) -> Result<LogLevel, &'static str> {
    match raw.to_lowercase().as_str() {
        "debug" => Ok(LogLevel::Debug),
        "info" => Ok(LogLevel::Info),
        "warning" => Ok(LogLevel::Warning),
        "error" => Ok(LogLevel::Error),
        _ => Err("invalid level"),
    }
}

fn bad_request(message: &str) -> axum::response::Response {
    (
        axum::http::StatusCode::BAD_REQUEST,
        axum::Json(serde_json::json!({ "error": message })),
    )
        .into_response()
}

fn query_error_response(error: LoggingError) -> axum::response::Response {
    if let LoggingError::InvalidQuery(message) = error {
        return bad_request(&message);
    }
    Logger::error(format!("telemetry query failed: {error}"));
    internal_error()
}

fn blocking_error_response(error: tokio::task::JoinError) -> axum::response::Response {
    Logger::error(format!("telemetry blocking query failed: {error}"));
    internal_error()
}

fn internal_error() -> axum::response::Response {
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        axum::Json(serde_json::json!({ "error": "internal server error" })),
    )
        .into_response()
}
