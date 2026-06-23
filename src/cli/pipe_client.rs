use std::io;

use marix_common::{
    ChatMessageInput, ChatMessageOutput, Pipe, PipeError, PipeResponse, SessionConfig, UserMessage,
};

#[derive(Debug)]
pub struct PipeClient {
    session_config: SessionConfig,
}

impl PipeClient {
    pub fn new(session_config: SessionConfig) -> Self {
        Self { session_config }
    }

    pub fn request(&mut self, input: ChatMessageInput) -> io::Result<ChatMessageOutput> {
        let _ = (input, &self.session_config);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            PipeError::Unavailable("pipe client implementation is not linked".to_owned()),
        ))
    }
}

impl Pipe for PipeClient {
    fn send(&mut self, _message: impl UserMessage) -> Result<PipeResponse, PipeError> {
        Err(PipeError::Unavailable(
            "pipe client send transport is not linked".to_owned(),
        ))
    }

    fn on_receive(&mut self, _message: impl UserMessage) -> Result<PipeResponse, PipeError> {
        Err(PipeError::Unavailable(
            "pipe client receive transport is not linked".to_owned(),
        ))
    }

    fn close(&mut self) -> Result<PipeResponse, PipeError> {
        self.on_close()
    }

    fn on_close(&mut self) -> Result<PipeResponse, PipeError> {
        Ok(PipeResponse::accepted())
    }
}
