use std::time::Duration;

use marix_common::{Config, LogSource, Logger};
use marix_host::HostSession;

fn main() {
    let config = match Config::load() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("failed to load config: {error}");
            std::process::exit(1);
        }
    };
    Logger::connect(LogSource::Host, Duration::from_secs(30))
        .expect("failed to connect to telemetry within 30s");
    Logger::log(format!("host '{}' logging configured", config.name));
    let mut session = HostSession::new(config.name);
    session.run();
    loop {
        std::thread::park();
    }
}
