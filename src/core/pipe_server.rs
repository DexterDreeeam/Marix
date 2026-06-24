use std::net::{Shutdown, TcpListener, TcpStream};

use marix_common::{
    read_pipe_message, write_pipe_message, ChatMessageInput, ChatMessageOutput, CompleteMessage,
    DynamicResponseSignal, Pipe, PipeError, PipeResponse, SessionConfig, UserMessage,
    UserMessageType,
};

use crate::model_backend::{ModelBackend, ModelBackendDeepseek};
use crate::preprocess::Preprocessor;
use crate::run_log;

#[derive(Debug)]
pub struct PipeServer {
    preprocessor: Preprocessor,
    model_backend: Box<dyn ModelBackend>,
    listener: TcpListener,
    stream: Option<TcpStream>,
    receive_error: Option<PipeError>,
    closed: bool,
}

impl PipeServer {
    pub fn new(session_config: SessionConfig) -> Result<Self, PipeError> {
        let model_backend = build_model_backend(&session_config)?;
        run_log::record(format!(
            "binding pipe server to {}",
            session_config.bind_address()
        ));
        let listener = TcpListener::bind(session_config.bind_address())
            .map_err(|error| PipeError::Unavailable(error.to_string()))?;
        run_log::record("pipe server listener is ready");
        Ok(Self {
            preprocessor: Preprocessor,
            model_backend,
            listener,
            stream: None,
            receive_error: None,
            closed: false,
        })
    }

    pub fn run(&mut self) -> Result<PipeResponse, PipeError> {
        while !self.closed {
            run_log::record("waiting for pipe client connection");
            let (stream, _) = self
                .listener
                .accept()
                .map_err(|error| PipeError::Unavailable(error.to_string()))?;
            match stream.peer_addr() {
                Ok(address) => run_log::record(format!("accepted pipe client from {address}")),
                Err(error) => {
                    run_log::record(format!("accepted pipe client with unknown peer: {error}"))
                }
            }
            self.stream = Some(stream);
            if let Err(error) = self.serve_connected_client() {
                run_log::record(format!("pipe client ended with error: {error}"));
                return Err(error);
            }
            run_log::record("pipe client disconnected");
            self.stream = None;
        }
        Ok(PipeResponse::accepted())
    }

    fn serve_connected_client(&mut self) -> Result<(), PipeError> {
        loop {
            let bytes = {
                if self.closed {
                    return Err(PipeError::ReceiveFailed(
                        "core pipe server is closed".to_owned(),
                    ));
                }
                let stream = self.active_stream_mut()?;
                match read_pipe_message(stream) {
                    Ok(bytes) => bytes,
                    Err(PipeError::ConnectionClosed) => {
                        run_log::record("pipe client connection closed");
                        return Ok(());
                    }
                    Err(error) => {
                        run_log::record(format!("failed to read pipe message: {error}"));
                        return Err(error);
                    }
                }
            };
            match UserMessageType::classify(&bytes)
                .map_err(|error| PipeError::ReceiveFailed(error.to_string()))?
            {
                UserMessageType::ChatMessageInput => {
                    let input = <ChatMessageInput as UserMessage>::from_bytes(&bytes)
                        .map_err(|error| PipeError::ReceiveFailed(error.to_string()))?;
                    run_log::record(format!(
                        "received chat input: correlation_id={}",
                        input.correlation_id()
                    ));
                    self.on_receive(input);
                    if let Some(error) = self.receive_error.take() {
                        run_log::record(format!("chat input failed: {error}"));
                        return Err(error);
                    }
                }
                UserMessageType::ChatMessageOutput | UserMessageType::CompleteMessage => {
                    run_log::record("rejected unexpected message type on pipe server");
                    return Err(PipeError::ReceiveFailed(
                        "pipe server can only receive ChatMessageInput".to_owned(),
                    ));
                }
            }
        }
    }

    fn active_stream_mut(&mut self) -> Result<&mut TcpStream, PipeError> {
        self.stream
            .as_mut()
            .ok_or_else(|| PipeError::Unavailable("pipe server has no active client".to_owned()))
    }

    fn handle_received_message(
        &mut self,
        message: impl UserMessage,
    ) -> Result<PipeResponse, PipeError> {
        if self.closed {
            return Err(PipeError::ReceiveFailed(
                "core pipe server is closed".to_owned(),
            ));
        }

        if message.get_type() != UserMessageType::ChatMessageInput {
            return Err(PipeError::ReceiveFailed(
                "core pipe input must be ChatMessageInput".to_owned(),
            ));
        }

        let correlation_id = message.correlation_id().to_owned();
        run_log::record(format!(
            "processing chat input: correlation_id={correlation_id}"
        ));
        let bytes = message
            .to_bytes()
            .map_err(|error| PipeError::ReceiveFailed(error.to_string()))?;
        let input = <ChatMessageInput as UserMessage>::from_bytes(&bytes)
            .map_err(|error| PipeError::ReceiveFailed(error.to_string()))?;
        let preprocessed = self
            .preprocessor
            .run(input)
            .map_err(|error| PipeError::ReceiveFailed(error.to_string()))?;
        run_log::record(format!(
            "preprocess completed: correlation_id={correlation_id}"
        ));
        let response = self
            .model_backend
            .request_response(preprocessed)
            .map_err(|error| PipeError::ReceiveFailed(error.to_string()))?;
        run_log::record(format!(
            "model response started: correlation_id={correlation_id}"
        ));
        let mut sent_length = 0;
        loop {
            match response.wait(None) {
                DynamicResponseSignal::Changed => {
                    let output = response.get();
                    if output.content.len() > sent_length {
                        let content = output.content[sent_length..].to_owned();
                        sent_length = output.content.len();
                        run_log::record(format!(
                            "sending output chunk: correlation_id={correlation_id}, bytes={}",
                            content.len()
                        ));
                        require_accepted(
                            self.send(ChatMessageOutput::new(correlation_id.clone(), content))?,
                        )?;
                    }
                }
                DynamicResponseSignal::Finished => {
                    let output = response.get();
                    if output.content.len() > sent_length {
                        let content = output.content[sent_length..].to_owned();
                        run_log::record(format!(
                            "sending final output chunk: correlation_id={correlation_id}, bytes={}",
                            content.len()
                        ));
                        require_accepted(
                            self.send(ChatMessageOutput::new(correlation_id.clone(), content))?,
                        )?;
                    }
                    run_log::record(format!(
                        "sending complete message: correlation_id={correlation_id}"
                    ));
                    return self.send(CompleteMessage::new(correlation_id));
                }
                DynamicResponseSignal::Failed(reason) => {
                    run_log::record(format!(
                        "model response failed: correlation_id={correlation_id}, reason={reason}"
                    ));
                    return Err(PipeError::ReceiveFailed(reason));
                }
                DynamicResponseSignal::TimedOut => {}
            }
        }
    }
}

fn require_accepted(response: PipeResponse) -> Result<(), PipeError> {
    match response {
        PipeResponse::Accepted => Ok(()),
        PipeResponse::Rejected(reason) => Err(PipeError::ReceiveFailed(reason)),
    }
}

fn build_model_backend(session_config: &SessionConfig) -> Result<Box<dyn ModelBackend>, PipeError> {
    let configured_backend = session_config.model_backend();
    run_log::record(format!("building model backend: {configured_backend}"));
    match configured_backend.trim().to_ascii_lowercase().as_str() {
        "deepseek" => Ok(Box::new(ModelBackendDeepseek::new())),
        _ => Err(PipeError::Unavailable(format!(
            "unsupported model backend: {configured_backend}"
        ))),
    }
}

impl Pipe for PipeServer {
    fn send(&mut self, message: impl UserMessage) -> Result<PipeResponse, PipeError> {
        if self.closed {
            return Err(PipeError::SendFailed(
                "core pipe server is closed".to_owned(),
            ));
        }

        match message.get_type() {
            UserMessageType::ChatMessageOutput => {
                let bytes = message
                    .to_bytes()
                    .map_err(|error| PipeError::SendFailed(error.to_string()))?;
                let output = <ChatMessageOutput as UserMessage>::from_bytes(&bytes)
                    .map_err(|error| PipeError::SendFailed(error.to_string()))?;
                if output.is_empty() {
                    return Ok(PipeResponse::rejected("core output message is empty"));
                }
                let stream = self.active_stream_mut()?;
                write_pipe_message(stream, &output)?;
                Ok(PipeResponse::accepted())
            }
            UserMessageType::CompleteMessage => {
                let bytes = message
                    .to_bytes()
                    .map_err(|error| PipeError::SendFailed(error.to_string()))?;
                let complete = <CompleteMessage as UserMessage>::from_bytes(&bytes)
                    .map_err(|error| PipeError::SendFailed(error.to_string()))?;
                if complete.is_empty() {
                    return Ok(PipeResponse::rejected(
                        "complete message correlation id is empty",
                    ));
                }
                let stream = self.active_stream_mut()?;
                write_pipe_message(stream, &complete)?;
                Ok(PipeResponse::accepted())
            }
            UserMessageType::ChatMessageInput => Ok(PipeResponse::rejected(
                "core pipe output must be ChatMessageOutput or CompleteMessage",
            )),
        }
    }

    fn on_receive(&mut self, message: impl UserMessage + Send + 'static) {
        if let Err(error) = self.handle_received_message(message) {
            self.receive_error = Some(error);
        }
    }

    fn close(&mut self) -> Result<PipeResponse, PipeError> {
        self.on_close()
    }

    fn on_close(&mut self) -> Result<PipeResponse, PipeError> {
        self.closed = true;
        run_log::record("closing pipe server");
        if let Some(stream) = self.stream.take() {
            stream
                .shutdown(Shutdown::Both)
                .map_err(|error| PipeError::Unavailable(error.to_string()))?;
        }
        Ok(PipeResponse::accepted())
    }
}
