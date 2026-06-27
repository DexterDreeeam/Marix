use std::io::{self, Write};
use std::net::SocketAddr;

use crate::client::core::ClientSession;
use crate::common::channel::ChannelError;
use crate::common::message::{ChatRequest, ResponseMessageEnvelope, ResponseMessageType};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliError {
    Channel(ChannelError),
    Output(String),
    UnexpectedResponse(ResponseMessageType),
}

pub fn run(request: CliRequest) -> Result<CliRunResult, CliError> {
    let mut session = ClientSession::connect(request.core_address).map_err(CliError::Channel)?;
    let result = run_task(&mut session, request.prompt);
    let close_result = session.close().map_err(CliError::Channel);
    match (result, close_result) {
        (Ok(result), Ok(())) => Ok(result),
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliRequest {
    pub core_address: SocketAddr,
    pub prompt: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CliRunResult {
    pub printed_segments: usize,
}

// -- Private -- //

fn run_task(session: &mut ClientSession, prompt: String) -> Result<CliRunResult, CliError> {
    let mut task = session
        .create_task(ChatRequest { content: prompt })
        .map_err(CliError::Channel)?;
    let mut output = io::stdout().lock();
    let mut printed_segments = 0;

    loop {
        match task.receive() {
            Ok(ResponseMessageEnvelope::ChatResponseSegment(segment)) => {
                output
                    .write_all(segment.content.as_bytes())
                    .map_err(output_error)?;
                output.flush().map_err(output_error)?;
                printed_segments += 1;
            }
            Err(ChannelError::Disconnected) => return Ok(CliRunResult { printed_segments }),
            Err(error) => return Err(CliError::Channel(error)),
        }
    }
}

fn output_error(error: io::Error) -> CliError {
    CliError::Output(error.to_string())
}
