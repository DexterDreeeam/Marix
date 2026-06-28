use std::net::SocketAddr;
use std::process;
use std::thread;
use std::time::Duration;

use marix::agent::frontdoor::AgentSession;
use marix::common::channel::ChannelError;
use marix::common::config::Config;

const ACTIVE_CLIENT_ERROR: &str = "agent session can only accept one client";
const ACCEPT_RETRY_DELAY: Duration = Duration::from_millis(50);

fn main() {
    if let Err(error) = run_agent() {
        eprintln!("{error}");
        process::exit(1);
    }
}

fn run_agent() -> Result<(), String> {
    let config = Config::load()?;
    if !config.agent.enabled {
        return Err("agent is disabled by configuration".to_owned());
    }
    let bind_address = parse_bind_address(&config.agent.bind_address)?;
    let mut session = AgentSession::new(bind_address).map_err(format_channel_error)?;
    serve_session(&mut session).map_err(format_channel_error)
}

fn parse_bind_address(value: &str) -> Result<SocketAddr, String> {
    value
        .parse()
        .map_err(|error| format!("invalid agent bind address '{value}': {error}"))
}

fn serve_session(session: &mut AgentSession) -> Result<(), ChannelError> {
    loop {
        match session.accept() {
            Ok(()) => {}
            Err(ChannelError::InvalidState(message)) if message == ACTIVE_CLIENT_ERROR => {
                thread::sleep(ACCEPT_RETRY_DELAY);
            }
            Err(error) => return Err(error),
        }
    }
}

fn format_channel_error(error: ChannelError) -> String {
    error.to_string()
}
