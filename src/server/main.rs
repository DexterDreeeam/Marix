use marix_common::{Config, Logger};
use marix_protocol::Actor;
use marix_server::Session;

fn main() {
    let config = match Config::load() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("failed to load config: {error}");
            std::process::exit(1);
        }
    };
    if !config.server.enabled {
        eprintln!("server is disabled by configuration");
        std::process::exit(1);
    }
    if let Err(error) = Logger::host() {
        eprintln!("failed to configure logger: {error}");
        std::process::exit(1);
    }
    if config.logging.remote {
        Logger::log(format!(
            "server telemetry hosting on port {}",
            config.server.telemetry_port
        ));
    } else {
        Logger::log(format!("server '{}' local logging configured", config.name));
    }
    Logger::log(format!("server core '{}' starting", config.name));
    let mut session = Session::new(config.name);
    session.start();
    loop {
        std::thread::park();
    }
}
