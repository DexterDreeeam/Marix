use std::io::{self, BufRead, Write};
use std::time::Duration;

use marix_client::{ClientEvent, ClientSession};
use marix_common::Config;

const IDLE_TIMEOUT: Duration = Duration::from_secs(30);

enum ClientMode {
    Interactive,
    Oneshot(String),
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mode = match args.as_slice() {
        [mode] if mode == "--interactive" => ClientMode::Interactive,
        [mode, request] if mode == "--oneshot" => ClientMode::Oneshot(request.clone()),
        _ => {
            print_help();
            return;
        }
    };

    let config = match Config::load() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("failed to load config: {error}");
            std::process::exit(1);
        }
    };
    let mut session = ClientSession::new(config.name);
    for _ in 0..100 {
        if session.is_connected() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    match mode {
        ClientMode::Interactive => run_interactive(&session),
        ClientMode::Oneshot(request) => run_oneshot(&session, &request),
    }
    session.close();
}

fn run_oneshot(session: &ClientSession, request: &str) {
    session.create_task(request.to_owned());
    drain_events(session);
}

fn run_interactive(session: &ClientSession) {
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
        run_oneshot(session, prompt);
    }
}

fn print_help() {
    println!("Usage:");
    println!("  marix-client-cli --interactive");
    println!("  marix-client-cli --oneshot \"request\"");
}

fn drain_events(session: &ClientSession) {
    let receiver = session.receiver();
    let mut last_signature_id = None;
    while let Ok(event) = receiver.recv_timeout(IDLE_TIMEOUT) {
        match event {
            ClientEvent::Common {
                signature_id,
                message,
            } => {
                print_event(&mut last_signature_id, signature_id, message);
                let _ = io::stdout().flush();
            }
            ClientEvent::Done {
                signature_id,
                message,
            } => {
                if let Some(message) = message {
                    print_event(&mut last_signature_id, signature_id, message);
                    let _ = io::stdout().flush();
                    break;
                }
                if last_signature_id.as_ref() == Some(&signature_id) {
                    break;
                }
            }
        }
    }
    if last_signature_id.is_some() {
        println!();
    }
}

fn print_event(last_signature_id: &mut Option<String>, signature_id: String, message: String) {
    if last_signature_id
        .as_ref()
        .is_some_and(|previous| previous != &signature_id)
    {
        println!();
    }
    print!("{message}");
    *last_signature_id = Some(signature_id);
}
