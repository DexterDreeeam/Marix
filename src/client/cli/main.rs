use std::io::{self, BufRead, Write};
use std::time::{Duration, Instant};

use marix_client::{ClientEvent, ClientSession};
use marix_common::{Config, LogSource, Logger};

const CONNECTION_POLL_INTERVAL: Duration = Duration::from_millis(25);

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
    match Logger::connect(LogSource::Client) {
        Ok(()) => {
            let status = if config.logging.remote {
                "connected to telemetry"
            } else {
                "local logging configured"
            };
            Logger::log(format!("client '{}' {status}", config.name));
        }
        Err(error) => {
            eprintln!("logger unavailable, continuing without it: {error}");
        }
    }
    let request_timeout = Duration::from_millis(config.client.request_timeout_ms);
    let mut session = ClientSession::new(config.name);
    if !wait_for_connection(&session, request_timeout) {
        let message = format!(
            "client connection timed out after {} ms",
            request_timeout.as_millis()
        );
        Logger::error(message.clone());
        eprintln!("{message}");
        session.close();
        return;
    }

    match mode {
        ClientMode::Interactive => run_interactive(&session, request_timeout),
        ClientMode::Oneshot(request) => {
            run_oneshot(&session, &request, request_timeout);
        }
    }
    session.close();
}

fn run_oneshot(session: &ClientSession, request: &str, request_timeout: Duration) -> bool {
    session.create_task(request.to_owned());
    drain_events(session, request_timeout)
}

fn run_interactive(session: &ClientSession, request_timeout: Duration) {
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let prompt = match line {
            Ok(prompt) => prompt,
            Err(error) => {
                Logger::error(format!("client stdin read failed: {error}"));
                eprintln!("failed to read stdin: {error}");
                break;
            }
        };
        let prompt = prompt.trim();
        if prompt.is_empty() {
            continue;
        }
        if !run_oneshot(session, prompt, request_timeout) {
            return;
        }
    }
}

fn print_help() {
    println!("Usage:");
    println!("  marix-client-cli --interactive");
    println!("  marix-client-cli --oneshot \"request\"");
}

fn wait_for_connection(session: &ClientSession, timeout: Duration) -> bool {
    let started = Instant::now();
    loop {
        if session.is_connected() {
            return true;
        }
        let remaining = timeout.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            return false;
        }
        std::thread::sleep(remaining.min(CONNECTION_POLL_INTERVAL));
    }
}

fn drain_events(session: &ClientSession, request_timeout: Duration) -> bool {
    let receiver = session.receiver();
    let mut printed = false;
    let started = Instant::now();
    let completed = loop {
        let remaining = request_timeout.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            break false;
        }
        let Ok(event) = receiver.recv_timeout(remaining) else {
            break false;
        };
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
                break true;
            }
        }
    };
    if printed {
        println!();
        let _ = io::stdout().flush();
    }
    if !completed {
        const MESSAGE: &str = "task response timed out";
        Logger::error(MESSAGE);
        eprintln!("{MESSAGE}");
    }
    completed
}
