use std::io::{self, BufRead, Write};
use std::time::Duration;

use marix_client::{ClientEvent, ClientSession};
use marix_common::Config;

const IDLE_TIMEOUT: Duration = Duration::from_secs(30);

fn main() {
    if let Err(error) = Config::load() {
        eprintln!("failed to load config: {error}");
        std::process::exit(1);
    }
    let mut session = ClientSession::new();
    for _ in 0..100 {
        if session.is_connected() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        session.create_task(args.join(" "));
        drain_events(&session);
        session.close();
        return;
    }

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let prompt = match line {
            Ok(prompt) => prompt,
            Err(error) => {
                eprintln!("failed to read stdin: {error}");
                break;
            }
        };
        let prompt = prompt.trim();
        if prompt.is_empty() {
            continue;
        }
        session.create_task(prompt.to_owned());
        drain_events(&session);
    }
    session.close();
}

fn drain_events(session: &ClientSession) {
    let receiver = session.receiver();
    while let Ok(ClientEvent::Common(message)) = receiver.recv_timeout(IDLE_TIMEOUT) {
        println!("{message}");
        let _ = io::stdout().flush();
    }
}
