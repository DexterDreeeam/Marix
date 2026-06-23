use std::io::{ErrorKind, Read, Write};

use super::{PipeError, PipeResponse};
use crate::message::UserMessage;

const FRAME_LENGTH_BYTES: usize = std::mem::size_of::<u64>();
const MAX_FRAME_BYTES: u64 = 16 * 1024 * 1024;

pub trait Pipe {
    fn send(&mut self, message: impl UserMessage) -> Result<PipeResponse, PipeError>;

    fn on_receive(&mut self, message: impl UserMessage + Send + 'static);

    fn close(&mut self) -> Result<PipeResponse, PipeError>;

    fn on_close(&mut self) -> Result<PipeResponse, PipeError>;
}

pub fn write_pipe_message(
    writer: &mut impl Write,
    message: &impl UserMessage,
) -> Result<(), PipeError> {
    let bytes = message
        .to_bytes()
        .map_err(|error| PipeError::SendFailed(error.to_string()))?;
    let length = u64::try_from(bytes.len())
        .map_err(|_| PipeError::SendFailed("message frame is too large".to_owned()))?;
    writer
        .write_all(&length.to_be_bytes())
        .and_then(|_| writer.write_all(&bytes))
        .and_then(|_| writer.flush())
        .map_err(|error| PipeError::SendFailed(error.to_string()))
}

pub fn read_pipe_message(reader: &mut impl Read) -> Result<Vec<u8>, PipeError> {
    let mut length_bytes = [0_u8; FRAME_LENGTH_BYTES];
    reader
        .read_exact(&mut length_bytes)
        .map_err(read_frame_length_error_to_pipe_error)?;
    let length = u64::from_be_bytes(length_bytes);
    if length > MAX_FRAME_BYTES {
        return Err(PipeError::ReceiveFailed(format!(
            "message frame is too large: {length} bytes"
        )));
    }
    let length = usize::try_from(length)
        .map_err(|_| PipeError::ReceiveFailed("message frame length is invalid".to_owned()))?;
    let mut bytes = vec![0_u8; length];
    reader
        .read_exact(&mut bytes)
        .map_err(|error| PipeError::ReceiveFailed(error.to_string()))?;
    Ok(bytes)
}

fn read_frame_length_error_to_pipe_error(error: std::io::Error) -> PipeError {
    match error.kind() {
        ErrorKind::UnexpectedEof
        | ErrorKind::ConnectionReset
        | ErrorKind::ConnectionAborted
        | ErrorKind::BrokenPipe => PipeError::ConnectionClosed,
        _ => PipeError::ReceiveFailed(error.to_string()),
    }
}
