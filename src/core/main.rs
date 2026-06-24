mod model_backend;
mod pipe_server;
mod preprocess;
mod run_log;

use std::io;

use marix_common::{PipeError, PipeResponse, SessionConfig};
use marix_config::config;

use crate::pipe_server::PipeServer;

fn main() -> io::Result<()> {
    let log_path = run_log::init()?;
    eprintln!("core log: {}", log_path.display());
    let session_config = SessionConfig::new(config.as_value());
    run_log::record(format!(
        "core starting with bind address {} and backend {}",
        session_config.bind_address(),
        session_config.model_backend()
    ));
    let result = PipeServer::new(session_config)
        .map_err(pipe_error_to_io)?
        .run()
        .map_err(pipe_error_to_io);
    if let Err(error) = &result {
        run_log::record(format!("core stopped with error: {error}"));
    } else {
        run_log::record("core stopped normally");
    }
    let response = result?;
    pipe_response_to_io_result(response)
}

fn pipe_error_to_io(error: PipeError) -> io::Error {
    io::Error::new(io::ErrorKind::Other, error)
}

fn pipe_response_to_io_result(response: PipeResponse) -> io::Result<()> {
    match response {
        PipeResponse::Accepted => Ok(()),
        PipeResponse::Rejected(reason) => Err(io::Error::new(io::ErrorKind::InvalidInput, reason)),
    }
}
