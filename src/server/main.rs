use marix_common::{Config, LogSource, Logger};
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
    let mut session = Session::new(config.name.clone());
    Logger::set_id(session.session_id());
    if let Err(error) = Logger::connect(LogSource::Server) {
        eprintln!("failed to configure logger: {error}");
        std::process::exit(1);
    }
    if config.logging.remote {
        Logger::log(format!("server '{}' connected to telemetry", config.name));
    } else {
        Logger::log(format!("server '{}' local logging configured", config.name));
    }
    Logger::log(format!("core session '{}' initializing", config.name));
    Logger::log(format!("server core '{}' starting", config.name));
    session.start();
    loop {
        std::thread::park();
    }
}
