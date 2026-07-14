use crate::http::external::*;
use crate::http::handlers;

pub(super) fn build() -> axum::Router {
    axum::Router::new()
        .route("/", axum::routing::get(handlers::root))
        .route("/favicon.svg", axum::routing::get(handlers::favicon))
        .route("/telemetry.css", axum::routing::get(handlers::stylesheet))
        .route(
            "/telemetry-data.js",
            axum::routing::get(handlers::data_script),
        )
        .route(
            "/telemetry-format.js",
            axum::routing::get(handlers::format_script),
        )
        .route(
            "/telemetry-message.js",
            axum::routing::get(handlers::message_script),
        )
        .route("/telemetry.js", axum::routing::get(handlers::script))
        .route("/api/sessions", axum::routing::get(handlers::sessions))
        .route("/api/logs", axum::routing::get(handlers::logs))
        .route("/api/logs/{id}", axum::routing::get(handlers::log_record))
        .layer(tower_http::compression::CompressionLayer::new())
}
