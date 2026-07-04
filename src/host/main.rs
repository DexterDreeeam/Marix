use marix_common::Config;
use marix_host::HostSession;

fn main() {
    let config = match Config::load() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("failed to load config: {error}");
            std::process::exit(1);
        }
    };
    let _session = HostSession::new(config.name);
    loop {
        std::thread::park();
    }
}
