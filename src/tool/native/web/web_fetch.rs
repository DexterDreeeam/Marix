use std::process::Command;

use marix_common::{
    Arch, Platform, System,
    external::serde_json::{Value, json, to_string},
};
use marix_protocol::{ToolCategory, ToolPreview};

use super::super::parse_input;
use crate::ToolProgram;

pub struct WebFetch;

impl WebFetch {
    const NAME: &'static str = "web_fetch";
    const DESCRIPTION: &'static str = "Fetch a URL from the internet and return the page content. Strips excessive HTML tags to return clean markdown-like text. Supports pagination via start_index/max_length; the response reports next_start_index and truncated when more content remains.";
    const INPUT_SCHEMA: &'static str = r#"{"type":"object","properties":{"url":{"type":"string"},"max_length":{"type":"integer","minimum":1,"maximum":15000},"start_index":{"type":"integer","minimum":0}},"required":["url"],"additionalProperties":false}"#;
}

const DEFAULT_MAX_LENGTH: usize = 5000;
const MAX_MAX_LENGTH: usize = 15000;

impl ToolProgram for WebFetch {
    fn preview(&self) -> ToolPreview {
        ToolPreview {
            name: Self::NAME.to_owned(),
            description: Self::DESCRIPTION.to_owned(),
            category: ToolCategory::Web,
            system: System {
                platform: Platform::All,
                arch: Arch::All,
            },
            input: Self::INPUT_SCHEMA.to_owned(),
        }
    }

    fn invoke(&self, call: &str) -> String {
        let input: Value = match parse_input(call) {
            Ok(value) => value,
            Err(error) => return failure(format!("invalid input: {error}")),
        };
        let Some(url) = input.get("url").and_then(Value::as_str) else {
            return failure("missing required field: url".to_owned());
        };
        let max_length = match input.get("max_length") {
            Some(value) => match value.as_u64() {
                Some(value) if (1..=MAX_MAX_LENGTH as u64).contains(&value) => value as usize,
                _ => {
                    return failure(format!(
                        "max_length must be an integer from 1 to {MAX_MAX_LENGTH}"
                    ));
                }
            },
            None => DEFAULT_MAX_LENGTH,
        };
        let start_index = match input.get("start_index") {
            Some(value) => match value.as_u64() {
                Some(value) => value as usize,
                None => return failure("start_index must be a non-negative integer".to_owned()),
            },
            None => 0,
        };

        match Command::new("curl")
            .args([
                "-sS",
                "-L",
                "--max-time",
                "60",
                "--retry",
                "3",
                "--retry-delay",
                "2",
            ])
            .arg(url)
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    let content = String::from_utf8_lossy(&output.stdout).into_owned();
                    let content = if looks_like_html(&content) {
                        clean_html(&content)
                    } else {
                        content
                    };
                    match paginate(&content, start_index, max_length) {
                        Ok(page) => to_string(&json!({
                            "content": page.content,
                            "start_index": page.start_index,
                            "next_start_index": page.next_start_index,
                            "truncated": page.truncated,
                            "total_length": page.total_length,
                        }))
                        .unwrap_or_default(),
                        Err(error) => failure(error),
                    }
                } else {
                    failure(format!(
                        "curl error: {}",
                        String::from_utf8_lossy(&output.stderr)
                    ))
                }
            }
            Err(e) => failure(format!("failed to execute curl: {e}")),
        }
    }
}

// -- Private -- //

fn looks_like_html(content: &str) -> bool {
    let lowercase = content.to_ascii_lowercase();
    lowercase.contains("<!doctype html")
        || [
            "html", "head", "body", "title", "meta", "link", "script", "style", "main", "article",
            "section", "header", "footer", "nav", "div", "p", "h1", "h2", "h3", "h4", "h5", "h6",
            "ul", "ol", "li", "a", "br", "table", "tr", "td",
        ]
        .iter()
        .any(|name| contains_tag(&lowercase, name))
}

fn contains_tag(content: &str, name: &str) -> bool {
    let mut offset = 0;

    while let Some(relative_start) = content[offset..].find('<') {
        let mut start = offset + relative_start + 1;
        let bytes = content.as_bytes();

        if bytes.get(start) == Some(&b'/') {
            start += 1;
        }
        while bytes.get(start).is_some_and(u8::is_ascii_whitespace) {
            start += 1;
        }

        let end = start + name.len();
        if content.get(start..end) == Some(name)
            && bytes
                .get(end)
                .is_some_and(|byte| byte.is_ascii_whitespace() || matches!(byte, b'/' | b'>'))
        {
            return true;
        }

        offset = start.min(content.len());
        if offset == content.len() {
            break;
        }
    }

    false
}

fn clean_html(content: &str) -> String {
    let lowercase = content.to_ascii_lowercase();
    let mut rendered = String::with_capacity(content.len());
    let mut offset = 0;
    let mut links: Vec<Option<(String, usize)>> = Vec::new();

    while offset < content.len() {
        let Some(relative_start) = content[offset..].find('<') else {
            rendered.push_str(&content[offset..]);
            break;
        };
        let start = offset + relative_start;

        rendered.push_str(&content[offset..start]);

        if content[start..].starts_with("<!--") {
            offset = content[start + 4..]
                .find("-->")
                .map_or(content.len(), |end| start + 4 + end + 3);
            continue;
        }

        let Some(end) = find_tag_end(content, start + 1) else {
            rendered.push_str(&content[start..]);
            break;
        };
        let tag = &content[start + 1..end];
        let Some((name, closing, self_closing)) = parse_tag(tag) else {
            rendered.push('<');
            offset = start + 1;
            continue;
        };

        if is_suppressed_element(name) {
            if !closing && !self_closing {
                offset = find_closing_element(&lowercase, end + 1, name)
                    .and_then(|start| find_tag_end(content, start + 2 + name.len()))
                    .map_or(content.len(), |end| end + 1);
            } else {
                offset = end + 1;
            }
            continue;
        }

        if name.eq_ignore_ascii_case("a") {
            if closing {
                if let Some(Some((href, text_start))) = links.pop() {
                    let text = rendered.split_off(text_start);
                    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
                    rendered.push('[');
                    rendered.push_str(&text);
                    rendered.push_str("](");
                    rendered.push_str(&href);
                    rendered.push(')');
                }
            } else if !self_closing {
                let href = extract_href(tag).map(|href| (href, rendered.len()));
                links.push(href);
            }
        }

        if is_block_element(name) {
            rendered.push('\n');
        }
        offset = end + 1;
    }

    normalize_lines(&decode_entities(&rendered))
}

fn find_closing_element(content: &str, start: usize, name: &str) -> Option<usize> {
    let pattern = format!("</{}", name.to_ascii_lowercase());
    let mut offset = start;

    while let Some(relative_start) = content[offset..].find(&pattern) {
        let tag_start = offset + relative_start;
        let name_end = tag_start + pattern.len();
        if content
            .as_bytes()
            .get(name_end)
            .is_some_and(|byte| byte.is_ascii_whitespace() || *byte == b'>')
        {
            return Some(tag_start);
        }
        offset = name_end;
    }

    None
}

fn find_tag_end(content: &str, start: usize) -> Option<usize> {
    let mut quote = None;

    for (relative_index, byte) in content.as_bytes()[start..].iter().enumerate() {
        match (*byte, quote) {
            (b'\'' | b'"', None) => quote = Some(*byte),
            (current, Some(expected)) if current == expected => quote = None,
            (b'>', None) => return Some(start + relative_index),
            _ => {}
        }
    }

    None
}

fn parse_tag(tag: &str) -> Option<(&str, bool, bool)> {
    let tag = tag.trim();
    if tag.starts_with(['!', '?']) {
        return Some(("", false, true));
    }

    let closing = tag.starts_with('/');
    let body = if closing { tag[1..].trim_start() } else { tag };
    let name_end = body
        .find(|character: char| !character.is_ascii_alphanumeric())
        .unwrap_or(body.len());
    if name_end == 0 {
        return None;
    }

    Some((&body[..name_end], closing, tag.ends_with('/')))
}

fn is_suppressed_element(name: &str) -> bool {
    ["script", "style", "svg", "noscript"]
        .iter()
        .any(|candidate| name.eq_ignore_ascii_case(candidate))
}

fn is_block_element(name: &str) -> bool {
    [
        "p", "div", "li", "article", "section", "header", "footer", "br", "tr",
    ]
    .iter()
    .any(|candidate| name.eq_ignore_ascii_case(candidate))
        || name.len() == 2
            && name.as_bytes()[0].eq_ignore_ascii_case(&b'h')
            && matches!(name.as_bytes()[1], b'1'..=b'6')
}

fn extract_href(tag: &str) -> Option<String> {
    let bytes = tag.as_bytes();
    let mut offset = bytes.iter().position(|byte| byte.is_ascii_whitespace())?;

    while offset < bytes.len() {
        while bytes.get(offset).is_some_and(u8::is_ascii_whitespace) {
            offset += 1;
        }
        if bytes.get(offset) == Some(&b'/') {
            break;
        }

        let name_start = offset;
        while bytes
            .get(offset)
            .is_some_and(|byte| !byte.is_ascii_whitespace() && !matches!(byte, b'=' | b'/' | b'>'))
        {
            offset += 1;
        }
        let attribute_name = &tag[name_start..offset];

        while bytes.get(offset).is_some_and(u8::is_ascii_whitespace) {
            offset += 1;
        }
        if bytes.get(offset) != Some(&b'=') {
            continue;
        }
        offset += 1;
        while bytes.get(offset).is_some_and(u8::is_ascii_whitespace) {
            offset += 1;
        }

        let (value_start, value_end) = match bytes.get(offset) {
            Some(quote @ (b'\'' | b'"')) => {
                offset += 1;
                let start = offset;
                while bytes.get(offset).is_some_and(|byte| byte != quote) {
                    offset += 1;
                }
                let end = offset;
                offset += usize::from(offset < bytes.len());
                (start, end)
            }
            Some(_) => {
                let start = offset;
                while bytes
                    .get(offset)
                    .is_some_and(|byte| !byte.is_ascii_whitespace() && !matches!(byte, b'/' | b'>'))
                {
                    offset += 1;
                }
                (start, offset)
            }
            None => return None,
        };

        if attribute_name.eq_ignore_ascii_case("href") {
            return Some(tag[value_start..value_end].to_owned());
        }
    }

    None
}

fn decode_entities(content: &str) -> String {
    let mut decoded = String::with_capacity(content.len());
    let mut offset = 0;

    while let Some(relative_start) = content[offset..].find('&') {
        let start = offset + relative_start;
        decoded.push_str(&content[offset..start]);

        let Some(relative_end) = content[start + 1..].find(';') else {
            decoded.push_str(&content[start..]);
            return decoded;
        };
        let end = start + 1 + relative_end;
        if end - start > 16 {
            decoded.push('&');
            offset = start + 1;
            continue;
        }

        let entity = &content[start + 1..end];
        if let Some(character) = decode_entity(entity) {
            decoded.push(character);
            offset = end + 1;
        } else {
            decoded.push('&');
            offset = start + 1;
        }
    }

    decoded.push_str(&content[offset..]);
    decoded
}

fn decode_entity(entity: &str) -> Option<char> {
    match entity {
        "amp" => Some('&'),
        "quot" => Some('"'),
        "#x27" | "#X27" | "#39" => Some('\''),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "nbsp" => Some(' '),
        _ => entity
            .strip_prefix("#x")
            .or_else(|| entity.strip_prefix("#X"))
            .and_then(|digits| u32::from_str_radix(digits, 16).ok())
            .or_else(|| {
                entity
                    .strip_prefix('#')
                    .and_then(|digits| digits.parse().ok())
            })
            .and_then(char::from_u32),
    }
}

fn normalize_lines(content: &str) -> String {
    let mut lines = Vec::new();

    for line in content.lines() {
        let line = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if line.is_empty() {
            if lines
                .last()
                .is_some_and(|previous: &String| !previous.is_empty())
            {
                lines.push(String::new());
            }
        } else {
            lines.push(line);
        }
    }

    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines.join("\n")
}

struct Page {
    content: String,
    start_index: usize,
    next_start_index: Option<usize>,
    truncated: bool,
    total_length: usize,
}

fn paginate(content: &str, start_index: usize, max_length: usize) -> Result<Page, String> {
    let chars: Vec<(usize, char)> = content.char_indices().collect();
    let total_length = chars.len();
    if start_index > total_length {
        return Err(format!(
            "start_index {start_index} is beyond the fetched content length {total_length}"
        ));
    }
    let end_index = (start_index + max_length).min(total_length);
    let byte_start = chars
        .get(start_index)
        .map_or(content.len(), |(byte, _)| *byte);
    let byte_end = chars
        .get(end_index)
        .map_or(content.len(), |(byte, _)| *byte);
    let truncated = end_index < total_length;
    Ok(Page {
        content: content[byte_start..byte_end].to_owned(),
        start_index,
        next_start_index: if truncated { Some(end_index) } else { None },
        truncated,
        total_length,
    })
}

fn failure(message: String) -> String {
    to_string(&json!({ "error": message })).unwrap_or_default()
}

#[cfg(feature = "web_fetch")]
pub use self::WebFetch as SelectedTool;
