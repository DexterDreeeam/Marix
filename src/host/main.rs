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
    Logger::connect(LogSource::Host).expect("failed to connect telemetry");
    Logger::log(format!("host '{}' logging configured", config.name));
    let mut session = HostSession::new(config.name);
    session.run();
    loop {
        std::thread::park();
    }
}
