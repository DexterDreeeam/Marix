mod error;
mod external;
mod handlers;
mod router;
mod server;

use marix_common::{Config, Logger};

fn main() {
    let config = match Config::load() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("failed to load config: {error}");
            std::process::exit(1);
        }
    };
    if !config.server.enabled {
        eprintln!("server telemetry is disabled by server configuration");
        std::process::exit(1);
    }
    if let Err(error) = Logger::host() {
        eprintln!("failed to start telemetry collector: {error}");
        std::process::exit(1);
    }
    Logger::log(format!(
        "server telemetry collector listening on port {}",
        config.server.telemetry_port
    ));
    if let Err(error) = server::serve(config.server.telemetry_http_port) {
        eprintln!("failed to start telemetry HTTP server: {error}");
        std::process::exit(1);
    }
}
