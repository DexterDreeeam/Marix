use std::env;
use std::io::{self, Read};

use marix_common::ToolLogger;
use marix_tool::{SelectedTool, ToolProgram};

fn main() {
    run(SelectedTool);
}

// A tool binary speaks JSON over standard streams: `--preview` prints the
// ToolPreview JSON, while normal execution reads a JSON payload from stdin and
// prints the JSON result.
fn run<T: ToolProgram>(tool: T) {
    let mut arguments = env::args().skip(1);
    match (arguments.next().as_deref(), arguments.next()) {
        (Some("--preview"), None) => match tool.preview().to_json() {
            Ok(json) => println!("{json}"),
            Err(error) => {
                eprintln!("failed to serialize tool preview: {error}");
                std::process::exit(1);
            }
        },
        (None, None) => {
            let mut logger = match ToolLogger::new() {
                Ok(logger) => logger,
                Err(error) => {
                    eprintln!("failed to create tool log: {error}");
                    std::process::exit(1);
                }
            };
            let mut input = String::new();
            if let Err(error) = io::stdin().read_to_string(&mut input) {
                let message = format!("error: failed to read tool input: {error}");
                if let Err(log_error) = logger.log(&message) {
                    eprintln!(
                        "failed to read tool input: {error}; \
                         additionally failed to log the error: {log_error}"
                    );
                } else {
                    eprintln!("failed to read tool input: {error}");
                }
                std::process::exit(1);
            }
            if let Err(error) = logger.log(&format!("input: {input}")) {
                eprintln!("failed to log tool input: {error}");
                std::process::exit(1);
            }

            let output = tool.invoke(&input);
            if let Err(error) = logger.log(&format!("output: {output}")) {
                eprintln!("failed to log tool output: {error}");
                std::process::exit(1);
            }
            println!("{output}");
        }
        _ => {
            eprintln!("usage: --preview | < JSON via stdin");
            std::process::exit(2);
        }
    }
}
