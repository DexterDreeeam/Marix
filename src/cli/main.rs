mod input;
mod interface;
mod output;
mod pipe_client;

use std::io;

use marix_common::SessionConfig;
use marix_config::config;

pub use input::ChatMessageInput;
pub use interface::{CliInterface, Interface};
pub use output::ChatMessageOutput;
pub use pipe_client::PipeClient;

fn main() -> io::Result<()> {
    let input = std::env::args().skip(1).collect::<Vec<_>>().join(" ");
    if !input.is_empty() {
        let output = run_session(ChatMessageInput::new(input))?;
        println!("{}", output.content());
    }
    Ok(())
}

fn run_session(input: ChatMessageInput) -> io::Result<ChatMessageOutput> {
    let session_config = SessionConfig::new(config.as_value());
    PipeClient::new(session_config).request(input)
}
