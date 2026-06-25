use std::fmt;
use std::net::{Shutdown, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use marix_common::{
    read_pipe_message, write_pipe_message, ChatMessageInput, ChatMessageOutput, CompleteMessage,
    Pipe, PipeError, PipeResponse, SessionConfig, UserMessage, UserMessageType,
};

type UserMessageBox = Box<dyn UserMessage + Send>;
type ReceiveHandler = Box<dyn FnMut(UserMessageBox) + Send + 'static>;
type SharedReceiveHandler = Arc<Mutex<ReceiveHandler>>;

pub struct PipeClient {
    stream: TcpStream,
    receive_handler: SharedReceiveHandler,
    receive_thread: Option<JoinHandle<Result<(), PipeError>>>,
    closed: bool,
}

impl PipeClient {
    pub fn new(
        session_config: SessionConfig,
        receive_handler: impl FnMut(UserMessageBox) + Send + 'static,
    ) -> Result<Self, PipeError> {
        let stream = TcpStream::connect(session_config.bind_address())
            .map_err(|error| PipeError::Unavailable(error.to_string()))?;
        let receive_stream = stream
            .try_clone()
            .map_err(|error| PipeError::Unavailable(error.to_string()))?;
        let receive_handler: SharedReceiveHandler = Arc::new(Mutex::new(Box::new(receive_handler)));
        let receive_thread_handler = Arc::clone(&receive_handler);
        let receive_thread =
            thread::spawn(move || run_receive_loop(receive_stream, receive_thread_handler));

        Ok(Self {
            stream,
            receive_handler,
            receive_thread: Some(receive_thread),
            closed: false,
        })
    }
}

impl Pipe for PipeClient {
    fn send(&mut self, message: impl UserMessage) -> Result<PipeResponse, PipeError> {
        if self.closed {
            return Err(PipeError::SendFailed("pipe client is closed".to_owned()));
        }
        write_pipe_message(&mut self.stream, &message)?;
        Ok(PipeResponse::accepted())
    }

    fn on_receive(&mut self, message: impl UserMessage + Send + 'static) {
        dispatch_received_message(&self.receive_handler, Box::new(message))
            .expect("pipe client receive handler failed");
    }

    fn close(&mut self) -> Result<PipeResponse, PipeError> {
        self.on_close()
    }

    fn on_close(&mut self) -> Result<PipeResponse, PipeError> {
        self.closed = true;
        self.stream
            .shutdown(Shutdown::Both)
            .map_err(|error| PipeError::Unavailable(error.to_string()))?;
        if let Some(receive_thread) = self.receive_thread.take() {
            match receive_thread.join() {
                Ok(result) => result?,
                Err(_) => {
                    return Err(PipeError::ReceiveFailed(
                        "pipe client receive thread panicked".to_owned(),
                    ));
                }
            }
        }
        Ok(PipeResponse::accepted())
    }
}

impl fmt::Debug for PipeClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PipeClient")
            .field("stream", &self.stream)
            .field("closed", &self.closed)
            .finish_non_exhaustive()
    }
}

fn run_receive_loop(
    mut receive_stream: TcpStream,
    receive_handler: SharedReceiveHandler,
) -> Result<(), PipeError> {
    loop {
        let bytes = match read_pipe_message(&mut receive_stream) {
            Ok(bytes) => bytes,
            Err(PipeError::ConnectionClosed) => return Ok(()),
            Err(error) => return Err(error),
        };
        dispatch_received_message(&receive_handler, parse_user_message(&bytes)?)?;
    }
}

fn parse_user_message(bytes: &[u8]) -> Result<UserMessageBox, PipeError> {
    match UserMessageType::classify(bytes)
        .map_err(|error| PipeError::ReceiveFailed(error.to_string()))?
    {
        UserMessageType::ChatMessageInput => {
            let input = <ChatMessageInput as UserMessage>::from_bytes(bytes)
                .map_err(|error| PipeError::ReceiveFailed(error.to_string()))?;
            Ok(Box::new(input))
        }
        UserMessageType::ChatMessageOutput => {
            let output = <ChatMessageOutput as UserMessage>::from_bytes(bytes)
                .map_err(|error| PipeError::ReceiveFailed(error.to_string()))?;
            Ok(Box::new(output))
        }
        UserMessageType::CompleteMessage => {
            let complete = <CompleteMessage as UserMessage>::from_bytes(bytes)
                .map_err(|error| PipeError::ReceiveFailed(error.to_string()))?;
            Ok(Box::new(complete))
        }
    }
}

fn dispatch_received_message(
    receive_handler: &SharedReceiveHandler,
    message: UserMessageBox,
) -> Result<(), PipeError> {
    receive_handler
        .lock()
        .map_err(|_| PipeError::ReceiveFailed("pipe receive handler lock poisoned".to_owned()))?(
        message,
    );
    Ok(())
}
