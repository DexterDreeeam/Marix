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
    connect_telemetry(&config);
    let _session = HostSession::new(config.name);
    loop {
        std::thread::park();
    }
}

// -- Private -- //

/// Connects telemetry on a best-effort basis. Telemetry is a diagnostic side
/// channel, so an unreachable or misconfigured server must not stop the host
/// from serving executions.
fn connect_telemetry(config: &Config) {
    match Logger::connect() {
        Ok(()) => {
            Logger::log(format!("host '{}' connected to telemetry", config.name));
        }
        Err(error) => {
            eprintln!("telemetry logger unavailable, continuing without it: {error}");
        }
    }
}
