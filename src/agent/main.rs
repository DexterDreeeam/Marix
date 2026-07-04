use marix_agent::Session;
use marix_common::Config;

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
    let _session = Session::new(config.name);
    loop {
        std::thread::park();
    }
}
