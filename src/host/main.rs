use marix_common::Config;
use marix_host::HostSession;

fn main() {
    if let Err(error) = Config::load() {
        eprintln!("failed to load config: {error}");
        std::process::exit(1);
    }
    let _session = HostSession::new();
    loop {
        std::thread::park();
    }
}
