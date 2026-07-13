use marix_common::Logger;

use crate::http::error::HttpError;
use crate::http::external::*;
use crate::http::router;

/// Binds `0.0.0.0:port` and serves the telemetry HTTP app, blocking the
/// calling thread. Returns an error immediately on bind failure, before
/// ever blocking, so the caller can report it and exit.
pub(crate) fn serve(port: u16) -> Result<(), HttpError> {
    let runtime = build_runtime()?;
    runtime.block_on(async move {
        let listener = bind(port).await?;
        serve_listener(listener).await
    })
}

// -- Private -- //

fn build_runtime() -> Result<tokio::runtime::Runtime, HttpError> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| HttpError::Runtime(error.to_string()))
}

/// Binds the HTTP listener. `port == 0` asks the OS for an ephemeral free
/// port, which tests use so they never contend for a fixed real port.
async fn bind(port: u16) -> Result<tokio::net::TcpListener, HttpError> {
    let address = format!("0.0.0.0:{port}");
    tokio::net::TcpListener::bind(&address)
        .await
        .map_err(|error| HttpError::Bind(error.to_string()))
}

/// Runs the telemetry app on an already-bound listener until the listener
/// itself fails; this future normally never resolves under a healthy OS
/// socket, which is what makes `serve` block the caller as intended.
async fn serve_listener(listener: tokio::net::TcpListener) -> Result<(), HttpError> {
    if let Ok(local_addr) = listener.local_addr() {
        Logger::log(format!(
            "server telemetry HTTP listening on port {}",
            local_addr.port()
        ));
    }
    let app = router::build();
    axum::serve(listener, app)
        .await
        .map_err(|error| HttpError::Serve(error.to_string()))
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::sync::Once;
    use std::time::Duration;

    use marix_common::Logger;

    use super::{bind, serve_listener};

    /// Writes a self-contained config fixture to the OS temp dir, points
    /// `MARIX_CONFIG` at it, and starts `Logger::host()` exactly once for
    /// this test binary, so `/api/sessions` and `/api/logs` have a real
    /// (temp-directory) local store to query instead of `NotHosting`.
    fn ensure_logger_hosted() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let marix_path = std::env::temp_dir()
                .join("marix-server-telemetry-test")
                .to_string_lossy()
                .replace('\\', "/");
            let fixture = format!(
                r#"
name = "marix-server-telemetry-test"

[runtime]
environment = "test"
mode = "network"
marix_path = "{marix_path}"

[client]
interactive = false
request_timeout_ms = 1000

[logging]
remote = false

[server]
enabled = true
ip = "127.0.0.1"
auth_token = "test-token"
client_port = 0
host_port = 0
telemetry_port = 0
telemetry_http_port = 0
max_turns = 8

[model]
backend = "deepseek"

[model.deepseek]
endpoint = "https://example.invalid/chat"
model = "deepseek-chat"
api_key = "test-key"

[tool]
directory = "tool"
"#
            );
            let path = std::env::temp_dir().join("marix_server_telemetry_test_config.toml");
            std::fs::write(&path, fixture).expect("write test config fixture");
            // SAFETY: only ever executed once, inside `Once::call_once`,
            // before any test reads the environment.
            unsafe {
                std::env::set_var("MARIX_CONFIG", &path);
            }
            Logger::host().expect("host local telemetry store for tests");
        });
    }

    /// Performs a minimal raw HTTP/1.1 GET against `address` and returns
    /// `(status_code, body)`. Avoids adding an HTTP client dependency for
    /// this smoke test; the server always closes after one response
    /// because every request sends `Connection: close`.
    fn http_get(address: std::net::SocketAddr, path: &str) -> (u16, String) {
        let mut stream = TcpStream::connect(address).expect("connect to test server");
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("set read timeout");
        let request =
            format!("GET {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n");
        stream.write_all(request.as_bytes()).expect("write request");
        let mut raw = String::new();
        stream.read_to_string(&mut raw).expect("read response");

        let mut parts = raw.splitn(2, "\r\n\r\n");
        let head = parts.next().unwrap_or_default();
        let body = parts.next().unwrap_or_default().to_owned();
        let status = head
            .lines()
            .next()
            .and_then(|status_line| status_line.split_whitespace().nth(1))
            .and_then(|code| code.parse::<u16>().ok())
            .unwrap_or(0);
        (status, body)
    }

    /// Binds an OS-assigned (random) port, spawns the app on it inside the
    /// current Tokio runtime, and returns the address to call and a handle
    /// to abort the server task once the test is done. The listener binds
    /// the wildcard `0.0.0.0` address (matching production), but clients
    /// cannot connect to `0.0.0.0` directly, so the returned address swaps
    /// in `127.0.0.1` with the OS-assigned port.
    async fn spawn_test_server() -> (std::net::SocketAddr, tokio::task::JoinHandle<()>) {
        ensure_logger_hosted();
        let listener = bind(0).await.expect("bind ephemeral port");
        let bound_port = listener.local_addr().expect("listener local_addr").port();
        let address = std::net::SocketAddr::from(([127, 0, 0, 1], bound_port));
        let handle = tokio::spawn(async move {
            let _ = serve_listener(listener).await;
        });
        (address, handle)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn root_route_serves_html_page() {
        let (address, handle) = spawn_test_server().await;

        let (status, body) = tokio::task::spawn_blocking(move || http_get(address, "/"))
            .await
            .expect("blocking http_get");

        handle.abort();
        assert_eq!(status, 200);
        assert!(body.contains("<html"), "body was: {body}");
        assert!(
            body.contains(r#"id="session-list""#),
            "missing session list element, body was: {body}"
        );
        assert!(
            body.contains(r#"id="tag-filter""#),
            "missing tag filter element, body was: {body}"
        );
        assert!(
            body.contains(r#"id="keyword-filter""#),
            "missing keyword filter element, body was: {body}"
        );
        assert!(
            body.contains(r#"id="log-context-menu""#),
            "missing log context menu, body was: {body}"
        );
        assert!(
            body.contains(r#"id="format-message-modal""#),
            "missing format message modal, body was: {body}"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn sessions_route_returns_json_array() {
        let (address, handle) = spawn_test_server().await;

        let (status, body) =
            tokio::task::spawn_blocking(move || http_get(address, "/api/sessions"))
                .await
                .expect("blocking http_get");

        handle.abort();
        assert_eq!(status, 200);
        let parsed: serde_json::Value =
            serde_json::from_str(&body).expect("sessions body is valid JSON");
        assert!(parsed.is_array(), "expected a JSON array, got: {body}");
        assert!(
            parsed
                .as_array()
                .into_iter()
                .flatten()
                .all(|session| { session.get("id").is_some() && session.get("emit_ts").is_some() }),
            "expected session summary objects, got: {body}"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn logs_route_rejects_invalid_session_and_tag() {
        let (address, handle) = spawn_test_server().await;

        let (missing_status, _) =
            tokio::task::spawn_blocking(move || http_get(address, "/api/logs"))
                .await
                .expect("blocking http_get");
        assert_eq!(missing_status, 400);

        let (bad_uuid_status, _) = tokio::task::spawn_blocking(move || {
            http_get(address, "/api/logs?session_id=not-a-uuid")
        })
        .await
        .expect("blocking http_get");
        assert_eq!(bad_uuid_status, 400);

        let (bad_tag_status, _) = tokio::task::spawn_blocking(move || {
            http_get(address, "/api/logs?session_id=unknown&tag=bogus")
        })
        .await
        .expect("blocking http_get");
        assert_eq!(bad_tag_status, 400);

        let (empty_tag_status, empty_tag_body) = tokio::task::spawn_blocking(move || {
            http_get(address, "/api/logs?session_id=unknown&tag=")
        })
        .await
        .expect("blocking http_get");
        assert_eq!(empty_tag_status, 200);
        let parsed: serde_json::Value =
            serde_json::from_str(&empty_tag_body).expect("logs body is valid JSON");
        assert!(
            parsed.is_array(),
            "expected a JSON array, got: {empty_tag_body}"
        );

        handle.abort();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn unknown_route_returns_404() {
        let (address, handle) = spawn_test_server().await;

        let (status, _) = tokio::task::spawn_blocking(move || http_get(address, "/does-not-exist"))
            .await
            .expect("blocking http_get");

        handle.abort();
        assert_eq!(status, 404);
    }
}
