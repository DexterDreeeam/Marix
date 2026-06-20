use std::io;

use marix_common::{SessionConfig, SessionPipe};
use marix_config::config;
use marix_core::{
    AgentCore, EchoModelBackend, PassthroughCliCoreTransport, PassthroughModelTransport,
    Preprocessor,
};

fn main() -> io::Result<()> {
    let session_config = SessionConfig::from_core_config_value(config.as_value());
    let listener = SessionPipe::listen_core(&session_config)?;
    let core = AgentCore::new(config.current().clone());

    loop {
        let mut pipe = listener.accept()?;
        let input = pipe.receive_input()?;
        let output = core
            .run(
                input,
                &PassthroughCliCoreTransport,
                &Preprocessor,
                &PassthroughModelTransport,
                &EchoModelBackend,
            )
            .map_err(|error| io::Error::new(io::ErrorKind::Other, error))?;
        pipe.send_output(output)?;
    }
}
