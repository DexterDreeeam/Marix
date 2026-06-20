mod input;
mod interface;
mod output;

use std::io;

use marix_common::{SessionConfig, SessionPipe};
use marix_config::config;
use marix_core::{
    AgentCore, EchoModelBackend, PassthroughCliCoreTransport, PassthroughModelTransport,
    Preprocessor,
};

pub use input::UserInput;
pub use interface::{CliInterface, Interface};
pub use output::UserOutput;

fn main() -> io::Result<()> {
    let input = std::env::args().skip(1).collect::<Vec<_>>().join(" ");
    if !input.is_empty() {
        let output = run_session(UserInput::new(input))?;
        println!("{}", output.content);
    }
    Ok(())
}

fn run_session(input: UserInput) -> io::Result<UserOutput> {
    let session_config = SessionConfig::from_cli_config_value(config.as_value());
    if session_config.remote_core {
        let mut pipe = SessionPipe::connect_core(&session_config)?;
        return pipe.request(input);
    }

    let (mut cli_pipe, mut core_pipe) = SessionPipe::integrate_core();
    cli_pipe.send_input(input)?;

    let core = AgentCore::new(config.current().clone());
    let core_input = core_pipe.receive_input()?;
    let output = core
        .run(
            core_input,
            &PassthroughCliCoreTransport,
            &Preprocessor,
            &PassthroughModelTransport,
            &EchoModelBackend,
        )
        .map_err(|error| io::Error::new(io::ErrorKind::Other, error))?;
    core_pipe.send_output(output)?;
    cli_pipe.receive_output()
}
