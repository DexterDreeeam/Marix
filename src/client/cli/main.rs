use std::env;
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
    let prompt = read_prompt(env::args().skip(1))?;
    let config = Config::load()?;
    let core_address = parse_core_address(&config.client.core_address)?;
    run(CliRequest {
        core_address,
        prompt,
    })
    .map(|_| ())
    .map_err(|error| format!("{error:?}"))
}

fn read_prompt(args: impl Iterator<Item = String>) -> Result<String, String> {
    let prompt = args.collect::<Vec<_>>().join(" ");
    if prompt.trim().is_empty() {
        return Err("usage: marix-cli <prompt>".to_owned());
    }
    Ok(prompt)
}

fn parse_core_address(value: &str) -> Result<SocketAddr, String> {
    value
        .parse()
        .map_err(|error| format!("invalid core address '{value}': {error}"))
}
