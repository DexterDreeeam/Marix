use std::io::{self, BufRead, Write};
use std::time::{Duration, Instant};

use marix_client::{ClientEvent, ClientSession};
use marix_common::{Config, LogSource, Logger, Receiver};

const CONNECTION_POLL_INTERVAL: Duration = Duration::from_millis(25);
const CONNECTION_TERMINATED_SIGNATURE: &str = "__marix_client_connection__";
const CONNECTION_TERMINATED_MESSAGE: &str =
    "client connection event stream terminated before task completion";
const EVENT_STREAM_TERMINATED_MESSAGE: &str =
    "client event stream terminated before task completion";
const TASK_RESPONSE_TIMEOUT_MESSAGE: &str = "task response timed out";
const TASK_CREATED_MESSAGE: &str = "task created";

enum ClientMode {
    Interactive,
    Oneshot {
        request: String,
        max_completion_time_secs: Option<u64>,
        max_relay_count: Option<u64>,
    },
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mode = match ClientMode::parse(&args) {
        Ok(mode) => mode,
        Err(error) => {
            eprintln!("argument error: {error}");
            print_help();
            std::process::exit(2);
        }
    };

    let config = match Config::load() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("failed to load config: {error}");
            std::process::exit(1);
        }
    };
    Logger::connect(LogSource::Client).expect("failed to connect telemetry");
    Logger::log(format!("client '{}' logging configured", config.name));
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
        std::process::exit(1);
    }

    let succeeded = match mode {
        ClientMode::Interactive => run_interactive(&session, request_timeout),
        ClientMode::Oneshot {
            request,
            max_completion_time_secs,
            max_relay_count,
        } => run_oneshot(
            &session,
            &request,
            max_completion_time_secs,
            max_relay_count,
        ),
    };
    session.close();
    if !succeeded {
        std::process::exit(1);
    }
}

fn run_oneshot(
    session: &ClientSession,
    request: &str,
    max_completion_time_secs: Option<u64>,
    max_relay_count: Option<u64>,
) -> bool {
    run_request(
        session,
        request,
        None,
        max_completion_time_secs,
        max_relay_count,
    )
}

fn run_request(
    session: &ClientSession,
    request: &str,
    response_timeout: Option<Duration>,
    max_completion_time_secs: Option<u64>,
    max_relay_count: Option<u64>,
) -> bool {
    session.create_task(
        request.to_owned(),
        max_completion_time_secs,
        max_relay_count,
    );
    let stdout = io::stdout();
    let result = drain_events(
        session.receiver(),
        || session.is_connected(),
        &mut stdout.lock(),
        response_timeout,
    );
    if let Err(message) = result {
        Logger::error(message.clone());
        eprintln!("{message}");
        return false;
    }
    true
}

fn run_interactive(session: &ClientSession, request_timeout: Duration) -> bool {
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let prompt = match line {
            Ok(prompt) => prompt,
            Err(error) => {
                Logger::error(format!("client stdin read failed: {error}"));
                eprintln!("failed to read stdin: {error}");
                return false;
            }
        };
        let prompt = prompt.trim();
        if prompt.is_empty() {
            continue;
        }
        if !run_request(session, prompt, Some(request_timeout), None, None) {
            return false;
        }
    }
    true
}

fn print_help() {
    println!("Usage:");
    println!("  marix-client-cli --interactive");
    println!(
        "  marix-client-cli --oneshot \"request\" \
         [--max-completion-time-secs <u64>] \
         [--max-relay-count <u64>]"
    );
}

impl ClientMode {
    fn parse(args: &[String]) -> Result<Self, String> {
        if matches!(args, [mode] if mode == "--interactive") {
            return Ok(Self::Interactive);
        }
        let [mode, request, remaining @ ..] = args else {
            return Err("expected --interactive or --oneshot \"request\"".to_owned());
        };
        if mode != "--oneshot" {
            return Err(format!("unknown mode `{mode}`"));
        }

        let mut max_completion_time_secs = None;
        let mut max_relay_count = None;
        let mut index = 0;
        while index < remaining.len() {
            let option = &remaining[index];
            let value = remaining
                .get(index + 1)
                .ok_or_else(|| format!("option `{option}` requires a <u64> value"))?;
            let value = value
                .parse::<u64>()
                .map_err(|_| format!("option `{option}` requires a valid <u64> value"))?;
            match option.as_str() {
                "--max-completion-time-secs" => {
                    if max_completion_time_secs.replace(value).is_some() {
                        return Err(format!("option `{option}` was provided more than once"));
                    }
                }
                "--max-relay-count" => {
                    if max_relay_count.replace(value).is_some() {
                        return Err(format!("option `{option}` was provided more than once"));
                    }
                }
                _ => return Err(format!("unknown option `{option}`")),
            }
            index += 2;
        }

        Ok(Self::Oneshot {
            request: request.clone(),
            max_completion_time_secs,
            max_relay_count,
        })
    }
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

fn drain_events(
    receiver: &Receiver<ClientEvent>,
    mut is_connected: impl FnMut() -> bool,
    output: &mut impl Write,
    response_timeout: Option<Duration>,
) -> Result<(), String> {
    let mut printed = false;
    let started = Instant::now();
    let result = loop {
        let wait = match response_timeout {
            Some(timeout) => {
                let remaining = timeout.saturating_sub(started.elapsed());
                if remaining.is_zero() {
                    break Err(TASK_RESPONSE_TIMEOUT_MESSAGE.to_owned());
                }
                remaining.min(CONNECTION_POLL_INTERVAL)
            }
            None => CONNECTION_POLL_INTERVAL,
        };
        let event = match receiver.recv_timeout(wait) {
            Ok(event) => event,
            Err(error) if error.is_disconnected() => {
                break Err(EVENT_STREAM_TERMINATED_MESSAGE.to_owned());
            }
            Err(_) if !is_connected() => {
                break Err(CONNECTION_TERMINATED_MESSAGE.to_owned());
            }
            Err(_) => continue,
        };
        match event {
            ClientEvent::Common { message, .. } => {
                if message == TASK_CREATED_MESSAGE {
                    continue;
                }
                let _ = write!(output, "{message}");
                let _ = output.flush();
                printed = true;
            }
            ClientEvent::Done {
                signature_id,
                message,
            } => {
                if signature_id == CONNECTION_TERMINATED_SIGNATURE {
                    break Err(message.unwrap_or_else(|| CONNECTION_TERMINATED_MESSAGE.to_owned()));
                }
                if let Some(message) = message {
                    let _ = write!(output, "{message}");
                    printed = true;
                }
                break Ok(());
            }
        }
    };
    if printed {
        let _ = writeln!(output);
        let _ = output.flush();
    }
    result
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread;

    use marix_common::build_channel;

    use super::*;

    #[test]
    fn delayed_final_event_has_no_response_timeout() {
        let (sender, receiver) = build_channel();
        let connected = Arc::new(AtomicBool::new(true));
        let worker = thread::spawn(move || {
            thread::sleep(Duration::from_millis(75));
            sender
                .send(ClientEvent::Done {
                    signature_id: String::new(),
                    message: Some("final answer".to_owned()),
                })
                .expect("send delayed final event");
        });
        let mut output = Vec::new();

        let result = drain_events(
            &receiver,
            || connected.load(Ordering::Relaxed),
            &mut output,
            None,
        );

        worker.join().expect("join delayed sender");
        assert_eq!(result, Ok(()));
        assert_eq!(output, b"final answer\n");
    }

    #[test]
    fn task_created_output_is_hidden() {
        let (sender, receiver) = build_channel();
        sender
            .send(ClientEvent::Common {
                signature_id: String::new(),
                message: TASK_CREATED_MESSAGE.to_owned(),
            })
            .expect("send task-created event");
        sender
            .send(ClientEvent::Done {
                signature_id: String::new(),
                message: Some("final answer".to_owned()),
            })
            .expect("send final event");
        let mut output = Vec::new();

        let result = drain_events(&receiver, || true, &mut output, None);

        assert_eq!(result, Ok(()));
        assert_eq!(output, b"final answer\n");
    }

    #[test]
    fn connection_termination_is_an_error() {
        let (sender, receiver) = build_channel();
        sender
            .send(ClientEvent::Done {
                signature_id: CONNECTION_TERMINATED_SIGNATURE.to_owned(),
                message: Some(CONNECTION_TERMINATED_MESSAGE.to_owned()),
            })
            .expect("send connection termination");
        let mut output = Vec::new();

        let result = drain_events(&receiver, || true, &mut output, None);

        assert_eq!(result, Err(CONNECTION_TERMINATED_MESSAGE.to_owned()));
        assert!(output.is_empty());
    }

    #[test]
    fn event_stream_termination_is_an_error() {
        let (sender, receiver) = build_channel::<ClientEvent>();
        drop(sender);
        let mut output = Vec::new();

        let result = drain_events(&receiver, || true, &mut output, None);

        assert_eq!(result, Err(EVENT_STREAM_TERMINATED_MESSAGE.to_owned()));
        assert!(output.is_empty());
    }

    #[test]
    fn only_first_final_result_is_printed() {
        let (sender, receiver) = build_channel();
        for message in ["first", "second"] {
            sender
                .send(ClientEvent::Done {
                    signature_id: String::new(),
                    message: Some(message.to_owned()),
                })
                .expect("send final event");
        }
        let mut output = Vec::new();

        let result = drain_events(&receiver, || true, &mut output, None);

        assert_eq!(result, Ok(()));
        assert_eq!(output, b"first\n");
    }
}
