mod model_backend;
mod pipe_server;
mod preprocess;

use std::io;

use marix_common::{PipeError, PipeResponse, SessionConfig};
use marix_config::config;

use crate::pipe_server::PipeServer;

fn main() -> io::Result<()> {
    let session_config = SessionConfig::new(config.as_value());
    let response = PipeServer::new(session_config)
        .map_err(pipe_error_to_io)?
        .run()
        .map_err(pipe_error_to_io)?;
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
