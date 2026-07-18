use std::process::Command;

use marix_common::external::serde_json::{Value, json, to_string};

use super::super::parse_input;

pub(super) fn invoke(call: &str, program: &str, arguments: &[&str]) -> String {
    let input: Value = match parse_input(call) {
        Ok(value) => value,
        Err(error) => return failure(format!("invalid input: {error}")),
    };
    let Some(command) = input.get("command").and_then(Value::as_str) else {
        return failure("missing required field: command".to_owned());
    };
    let cwd = input.get("cwd").and_then(Value::as_str);

    let mut process = Command::new(program);
    process.args(arguments).arg(command);
    if let Some(cwd) = cwd {
        process.current_dir(cwd);
    }
    match process.output() {
        Ok(output) => to_string(&json!({
            "exit_code": output.status.code().unwrap_or(-1),
            "stdout": String::from_utf8_lossy(&output.stdout).into_owned(),
            "stderr": String::from_utf8_lossy(&output.stderr).into_owned(),
        }))
        .unwrap_or_default(),
        Err(error) => failure(format!("failed to run command: {error}")),
    }
}

pub(super) fn unavailable(name: &str, required_platform: &str) -> String {
    failure(format!(
        "{name} is unavailable on this platform; \
         it requires {required_platform}"
    ))
}

// -- Private -- //

fn failure(message: String) -> String {
    to_string(&json!({ "error": message })).unwrap_or_default()
}
