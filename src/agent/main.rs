use marix_agent::Session;
use marix_common::{Config, Logger};

fn main() {
    let config = match Config::load() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("failed to load config: {error}");
            std::process::exit(1);
        }
    };
    if !config.agent.enabled {
        eprintln!("agent is disabled by configuration");
        std::process::exit(1);
    }
    if let Err(error) = Logger::host(config.telemetry.bind_port) {
        eprintln!("failed to start telemetry logger: {error}");
        std::process::exit(1);
    }
    let _ = Logger::log(format!(
        "agent telemetry hosting on port {}",
        config.telemetry.bind_port
    ));
    let _ = Logger::log(format!("agent core '{}' starting", config.name));
    let _session = Session::new(config.name);
    loop {
        std::thread::park();
    }
}
