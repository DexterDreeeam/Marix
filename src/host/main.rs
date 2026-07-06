use std::net::SocketAddr;

use marix_common::{Config, Logger};
use marix_host::HostSession;

fn main() {
    let config = match Config::load() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("failed to load config: {error}");
            std::process::exit(1);
        }
    };
    let telemetry_address = match config.telemetry.server_address.parse::<SocketAddr>() {
        Ok(address) => address,
        Err(error) => {
            eprintln!("invalid telemetry server address: {error}");
            std::process::exit(1);
        }
    };
    if let Err(error) = Logger::connect(telemetry_address) {
        eprintln!("failed to connect telemetry logger: {error}");
        std::process::exit(1);
    }
    let _ = Logger::log(format!("host '{}' connected to telemetry", config.name));
    let _session = HostSession::new(config.name);
    loop {
        std::thread::park();
    }
}
