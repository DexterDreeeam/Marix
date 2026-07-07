use std::io::{self, BufRead, Write};
use std::net::SocketAddr;
use std::time::Duration;

use marix_client::{ClientEvent, ClientSession};
use marix_common::{Config, Logger};

const IDLE_TIMEOUT: Duration = Duration::from_secs(120);

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
    let telemetry_address = format!("{}:{}", config.server.ip, config.server.telemetry_port);
    match telemetry_address.parse::<SocketAddr>() {
        Ok(address) => match Logger::connect(address) {
            Ok(()) => {
                let _ = Logger::log(format!("client '{}' connected to telemetry", config.name));
            }
            Err(error) => {
                eprintln!("telemetry logger unavailable, continuing without it: {error}");
            }
        },
        Err(error) => {
            eprintln!("invalid telemetry server address, continuing without telemetry: {error}");
        }
    }
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
                let _ = Logger::error(format!("client stdin read failed: {error}"));
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
    let mut printed = false;
    while let Ok(event) = receiver.recv_timeout(IDLE_TIMEOUT) {
        match event {
            ClientEvent::Common { message, .. } => {
                print!("{message}");
                let _ = io::stdout().flush();
                printed = true;
            }
            ClientEvent::Done { message, .. } => {
                if let Some(message) = message {
                    print!("{message}");
                    printed = true;
                }
                break;
            }
        }
    }
    if printed {
        println!();
        let _ = io::stdout().flush();
    }
}
