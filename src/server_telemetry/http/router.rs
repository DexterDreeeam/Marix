use crate::http::external::*;
use crate::http::handlers;

pub(super) fn build() -> axum::Router {
    axum::Router::new()
        .route("/", axum::routing::get(handlers::root))
        .route("/favicon.svg", axum::routing::get(handlers::favicon))
        .route("/api/sessions", axum::routing::get(handlers::sessions))
        .route("/api/logs", axum::routing::get(handlers::logs))
}
