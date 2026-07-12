mod file;
mod shell;
mod sys;

use marix_common::external::serde_json::{Error, Value, from_str};

pub use file::*;
pub use shell::*;
pub use sys::*;

// -- Private -- //

fn parse_input(call: &str) -> Result<Value, Error> {
    match from_str(call) {
        Ok(value) => Ok(value),
        Err(error) => {
            let Some(repaired) = repair_windows_path_fields(call) else {
                return Err(error);
            };
            from_str(&repaired)
        }
    }
}

fn repair_windows_path_fields(call: &str) -> Option<String> {
    let source = call.as_bytes();
    let mut repaired = Vec::with_capacity(source.len());
    let mut copied_until = 0;
    let mut cursor = 0;
    let mut changed = false;

    while cursor < source.len() {
        if source[cursor] != b'"' {
            cursor += 1;
            continue;
        }
        let Some(key_end) = string_end(source, cursor) else {
            break;
        };
        let key = &source[cursor + 1..key_end];
        if key != b"path" && key != b"cwd" {
            cursor = key_end + 1;
            continue;
        }

        let mut value_start = skip_whitespace(source, key_end + 1);
        if source.get(value_start) != Some(&b':') {
            cursor = key_end + 1;
            continue;
        }
        value_start = skip_whitespace(source, value_start + 1);
        if source.get(value_start) != Some(&b'"') {
            cursor = key_end + 1;
            continue;
        }

        repaired.extend_from_slice(&source[copied_until..=value_start]);
        cursor = value_start + 1;
        while cursor < source.len() {
            match source[cursor] {
                b'"' => {
                    repaired.push(b'"');
                    cursor += 1;
                    copied_until = cursor;
                    break;
                }
                b'\\' => {
                    repaired.push(b'\\');
                    match source.get(cursor + 1) {
                        Some(b'\\' | b'"') => {
                            repaired.push(source[cursor + 1]);
                            cursor += 2;
                        }
                        Some(next) => {
                            repaired.push(b'\\');
                            repaired.push(*next);
                            cursor += 2;
                            changed = true;
                        }
                        None => {
                            repaired.push(b'\\');
                            cursor += 1;
                            changed = true;
                        }
                    }
                }
                byte => {
                    repaired.push(byte);
                    cursor += 1;
                }
            }
        }
        if cursor == source.len() {
            copied_until = cursor;
        }
    }

    if !changed {
        return None;
    }
    repaired.extend_from_slice(&source[copied_until..]);
    String::from_utf8(repaired).ok()
}

fn string_end(source: &[u8], start: usize) -> Option<usize> {
    let mut cursor = start + 1;
    while cursor < source.len() {
        match source[cursor] {
            b'"' => return Some(cursor),
            b'\\' => cursor += 2,
            _ => cursor += 1,
        }
    }
    None
}

fn skip_whitespace(source: &[u8], mut cursor: usize) -> usize {
    while source
        .get(cursor)
        .is_some_and(|byte| byte.is_ascii_whitespace())
    {
        cursor += 1;
    }
    cursor
}
