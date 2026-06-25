mod input;
mod interface;
mod output;
mod pipe_client;

use std::io::{self, BufRead, Write};
use std::sync::mpsc::{self, Receiver};

use marix_common::{
    CompleteMessage, Pipe, PipeResponse, SessionConfig, UserMessage, UserMessageType,
};
use marix_config::config;

pub use input::ChatMessageInput;
pub use interface::{CliInterface, Interface};
pub use output::ChatMessageOutput;
pub use pipe_client::PipeClient;

fn main() -> io::Result<()> {
    run_session()
}

fn run_session() -> io::Result<()> {
    let session_config = SessionConfig::new(config.as_value());
    let (message_sender, message_receiver) = mpsc::channel();
    let receive_handler = move |message| {
        message_sender
            .send(message)
            .expect("pipe receive handler failed to forward message");
    };
    let mut pipe_client =
        PipeClient::new(session_config, receive_handler).map_err(pipe_error_to_io)?;
    let mut stdin = io::stdin().lock();
    let mut stdout = io::stdout().lock();
    let mut input = String::new();

    loop {
        input.clear();
        write!(stdout, "> ")?;
        stdout.flush()?;
        if stdin.read_line(&mut input)? == 0 {
            break;
        }
        let text = input.trim();
        if text.is_empty() {
            continue;
        }
        if matches!(text, "exit" | "quit") {
            break;
        }
        let message = ChatMessageInput::new(text.to_owned());
        let correlation_id = message.correlation_id().to_owned();
        pipe_response_to_io_result(pipe_client.send(message).map_err(pipe_error_to_io)?)?;
        print_until_complete(&message_receiver, &correlation_id, &mut stdout)?;
    }

    pipe_response_to_io_result(pipe_client.close().map_err(pipe_error_to_io)?)?;
    Ok(())
}

fn print_until_complete(
    message_receiver: &Receiver<Box<dyn UserMessage + Send>>,
    correlation_id: &str,
    stdout: &mut impl Write,
) -> io::Result<()> {
    loop {
        let message = message_receiver.recv().map_err(|_| {
            io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "pipe receive thread closed before complete message",
            )
        })?;
        match message.get_type() {
            UserMessageType::ChatMessageOutput => {
                let output = chat_message_output_from_user_message(message.as_ref())?;
                if output.correlation_id() != correlation_id {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "pipe output correlation id does not match input",
                    ));
                }
                stdout.write_all(output.content().as_bytes())?;
                stdout.flush()?;
            }
            UserMessageType::CompleteMessage => {
                let complete = complete_message_from_user_message(message.as_ref())?;
                if complete.correlation_id() != correlation_id {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "pipe complete correlation id does not match input",
                    ));
                }
                writeln!(stdout)?;
                stdout.flush()?;
                return Ok(());
            }
            UserMessageType::ChatMessageInput => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "pipe client cannot receive ChatMessageInput",
                ));
            }
        }
    }
}

fn pipe_response_to_io_result(response: PipeResponse) -> io::Result<()> {
    match response {
        PipeResponse::Accepted => Ok(()),
        PipeResponse::Rejected(reason) => Err(io::Error::new(io::ErrorKind::InvalidInput, reason)),
    }
}

fn pipe_error_to_io(error: marix_common::PipeError) -> io::Error {
    io::Error::new(io::ErrorKind::Other, error)
}

fn chat_message_output_from_user_message(
    message: &dyn UserMessage,
) -> io::Result<ChatMessageOutput> {
    let bytes = message.to_bytes().map_err(protocol_error_to_io)?;
    <ChatMessageOutput as UserMessage>::from_bytes(&bytes).map_err(protocol_error_to_io)
}

fn complete_message_from_user_message(message: &dyn UserMessage) -> io::Result<CompleteMessage> {
    let bytes = message.to_bytes().map_err(protocol_error_to_io)?;
    <CompleteMessage as UserMessage>::from_bytes(&bytes).map_err(protocol_error_to_io)
}

fn protocol_error_to_io(error: impl std::error::Error + Send + Sync + 'static) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}
