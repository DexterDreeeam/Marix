mod core_pipe_server;
mod model_backend;
mod model_backend_deepseek;
mod preprocess;

use std::io;

use crate::core_pipe_server::CorePipeServer;
use marix_common::{ChatMessageInput, PipeError, PipeResponse, PipeServer};

fn main() -> io::Result<()> {
    let input = std::env::args().skip(1).collect::<Vec<_>>().join(" ");
    if input.is_empty() {
        return Ok(());
    }

    let mut pipe = CorePipeServer::default();
    let response = pipe
        .on_receive(ChatMessageInput::new(input))
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
