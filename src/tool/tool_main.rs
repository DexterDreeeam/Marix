use std::env;

use marix_tool::{SelectedTool, ToolProgram};

fn main() {
    run(SelectedTool);
}

// A tool binary speaks JSON over stdout: `--preview` prints the ToolPreview JSON
// so the host can discover the tool, and `--run <input>` invokes the tool with
// the given JSON input payload and prints the JSON result.
fn run<T: ToolProgram>(tool: T) {
    let arguments: Vec<String> = env::args().collect();
    match arguments.get(1).map(String::as_str) {
        Some("--preview") => match tool.preview().to_json() {
            Ok(json) => println!("{json}"),
            Err(error) => {
                eprintln!("failed to serialize tool preview: {error}");
                std::process::exit(1);
            }
        },
        Some("--run") => {
            let input = arguments.get(2).map(String::as_str).unwrap_or_default();
            println!("{}", tool.invoke(input));
        }
        _ => {
            eprintln!("usage: --preview | --run <input>");
            std::process::exit(2);
        }
    }
}
