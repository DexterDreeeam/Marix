use std::env;
use std::io::{self, Read};

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
            let mut input = String::new();
            if let Err(error) = io::stdin().read_to_string(&mut input) {
                eprintln!("failed to read tool input: {error}");
                std::process::exit(1);
            }
            println!("{}", tool.invoke(&input));
        }
        _ => {
            eprintln!("usage: --preview | < JSON via stdin");
            std::process::exit(2);
        }
    }
}
