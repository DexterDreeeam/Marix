use std::env;
use std::io::{self, BufRead, Write};
use std::net::SocketAddr;
use std::process;

use marix::client::cli::{run, CliRequest};
use marix::common::config::Config;

fn main() {
    if let Err(error) = run_cli() {
        eprintln!("{error}");
        process::exit(1);
    }
}

fn run_cli() -> Result<(), String> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let config = Config::load()?;
    let core_address = parse_core_address(&config.client.core_address)?;
    if args.is_empty() {
        run_interactive(core_address)
    } else {
        run_prompt(core_address, args.join(" "))
    }
}

fn run_interactive(core_address: SocketAddr) -> Result<(), String> {
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let prompt = line.map_err(|error| format!("failed to read stdin: {error}"))?;
        if prompt.trim().is_empty() {
            continue;
        }
        if let Err(error) = run_prompt(core_address, prompt) {
            eprintln!("{error}");
            continue;
        }
        io::stdout()
            .write_all(b"\n")
            .map_err(|error| format!("failed to write stdout: {error}"))?;
    }
    Ok(())
}

fn run_prompt(core_address: SocketAddr, prompt: String) -> Result<(), String> {
    run(CliRequest {
        core_address,
        prompt,
    })
    .map(|_| ())
    .map_err(|error| format!("{error:?}"))
}

fn parse_core_address(value: &str) -> Result<SocketAddr, String> {
    value
        .parse()
        .map_err(|error| format!("invalid core address '{value}': {error}"))
}
