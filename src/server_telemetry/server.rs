use marix_common::Logger;

use crate::error::HttpError;
use crate::external::*;
use crate::router;

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
