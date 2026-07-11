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
    configure_logging(&config);
    let mut session = HostSession::new(config.name);
    session.run();
    loop {
        std::thread::park();
    }
}

// -- Private -- //

/// Configures logging on a best-effort basis. Diagnostics must not stop the
/// host from serving executions.
fn configure_logging(config: &Config) {
    match Logger::connect() {
        Ok(()) => {
            let status = if config.logging.remote {
                "connected to telemetry"
            } else {
                "local logging configured"
            };
            Logger::log(format!("host '{}' {status}", config.name));
        }
        Err(error) => {
            eprintln!("logger unavailable, continuing without it: {error}");
        }
    }
}
