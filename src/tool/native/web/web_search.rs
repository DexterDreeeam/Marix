use std::process::Command;

use marix_common::{
    Arch, Platform, System,
    external::serde_json::{Value, json, to_string},
};
use marix_protocol::{ToolCategory, ToolPreview};

use super::super::parse_input;
use crate::ToolProgram;

pub struct WebSearch;

impl WebSearch {
    const NAME: &'static str = "web_search";
    const DESCRIPTION: &'static str =
        "Search the web and return result titles, URLs, and snippets.";
    const INPUT_SCHEMA: &'static str = r#"{"type":"object","properties":{"query":{"type":"string","minLength":1},"max_results":{"type":"integer","minimum":1,"maximum":10}},"required":["query"],"additionalProperties":false}"#;
    const MAX_RESULTS: usize = 10;
}

impl ToolProgram for WebSearch {
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
            Err(error) => return Self::failure(format!("invalid input: {error}")),
        };
        let Some(query) = input.get("query").and_then(Value::as_str) else {
            return Self::failure("missing required field: query".to_owned());
        };
        if query.trim().is_empty() {
            return Self::failure("query must not be empty".to_owned());
        }
        let max_results = match input.get("max_results") {
            Some(value) => match value.as_u64() {
                Some(value) if (1..=Self::MAX_RESULTS as u64).contains(&value) => value as usize,
                _ => {
                    return Self::failure(format!(
                        "max_results must be an integer from 1 to {}",
                        Self::MAX_RESULTS
                    ));
                }
            },
            None => Self::MAX_RESULTS,
        };
        match Self::search(query, max_results) {
            Ok(results) => to_string(&json!({ "results": results })).unwrap_or_default(),
            Err(error) => Self::failure(error),
        }
    }
}

#[cfg(feature = "web_search")]
pub use self::WebSearch as SelectedTool;

// -- Private -- //

impl WebSearch {
    fn search(query: &str, max_results: usize) -> Result<Vec<Value>, String> {
        let mut last_error = String::new();

        match Self::search_duckduckgo(query, max_results) {
            Ok(results) if !results.is_empty() => return Ok(results),
            Ok(_) => last_error.push_str("DuckDuckGo returned 0 results. "),
            Err(e) => {
                last_error.push_str(&e);
                last_error.push_str(". ");
            }
        }

        match Self::search_yahoo(query, max_results) {
            Ok(results) if !results.is_empty() => return Ok(results),
            Ok(_) => last_error.push_str("Yahoo returned 0 results. "),
            Err(e) => {
                last_error.push_str(&e);
                last_error.push_str(". ");
            }
        }

        match Self::search_wikipedia(query, max_results) {
            Ok(results) if !results.is_empty() => return Ok(results),
            Ok(_) => last_error.push_str("Wikipedia returned 0 results."),
            Err(e) => last_error.push_str(&e),
        }

        Err(format!(
            "All search engines failed. Errors: {}",
            last_error.trim()
        ))
    }

    fn search_duckduckgo(query: &str, max_results: usize) -> Result<Vec<Value>, String> {
        let url = format!(
            "https://html.duckduckgo.com/html/?q={}",
            Self::percent_encode(query.as_bytes())
        );
        let output = Command::new("curl")
            .args([
                "-sS",
                "-L",
                "--proto",
                "=https",
                "--max-time",
                "60",
                "--retry",
                "3",
                "--retry-delay",
                "2",
                "--max-filesize",
                "1048576",
                "--user-agent",
                "Marix/1.0",
            ])
            .arg(url)
            .output()
            .map_err(|error| format!("failed to execute curl: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "DuckDuckGo request failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }

        let page = String::from_utf8_lossy(&output.stdout);
        Ok(Self::parse_duckduckgo_results(&page, max_results))
    }

    fn parse_duckduckgo_results(page: &str, max_results: usize) -> Vec<Value> {
        let mut results = Vec::new();
        let mut remaining = page;

        while results.len() < max_results {
            let Some(anchor_start) = remaining.find("result__a") else {
                break;
            };
            remaining = &remaining[anchor_start..];
            let Some(href_start) = remaining.find("href=\"") else {
                break;
            };
            let href = &remaining[href_start + 6..];
            let Some(href_end) = href.find('"') else {
                break;
            };
            let href = &href[..href_end];
            let Some(title_start) = remaining[href_start + 6 + href_end..].find('>') else {
                break;
            };
            let anchor_content = &remaining[href_start + 6 + href_end + title_start + 1..];
            let Some(title_end) = anchor_content.find("</a>") else {
                break;
            };
            let title = Self::clean_html(&anchor_content[..title_end]);
            let url = Self::result_url(href);
            let after_anchor = &anchor_content[title_end + 4..];
            let next_result = after_anchor.find("result__a").unwrap_or(after_anchor.len());
            let result_area = &after_anchor[..next_result];
            let snippet = Self::snippet(result_area);

            if !title.is_empty() && !url.is_empty() {
                results.push(json!({
                    "title": title,
                    "url": url,
                    "snippet": snippet,
                }));
            }
            remaining = &after_anchor[next_result..];
        }

        results
    }

    fn search_yahoo(query: &str, max_results: usize) -> Result<Vec<Value>, String> {
        let url = format!(
            "https://search.yahoo.com/search?p={}",
            Self::percent_encode(query.as_bytes())
        );
        let output = Command::new("curl")
            .args([
                "-sS",
                "-L",
                "--max-time",
                "60",
                "--retry",
                "3",
                "--retry-delay",
                "2",
                "--user-agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            ])
            .arg(url)
            .output()
            .map_err(|error| format!("failed to execute curl: {error}"))?;

        if !output.status.success() {
            return Err(format!(
                "Yahoo request failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }

        let page = String::from_utf8_lossy(&output.stdout);
        Ok(Self::parse_yahoo_results(&page, max_results))
    }

    fn parse_yahoo_results(page: &str, max_results: usize) -> Vec<Value> {
        let mut results = Vec::new();
        let mut remaining = page;

        while results.len() < max_results {
            let Some(h3_start) = remaining.find("<h3") else {
                break;
            };
            remaining = &remaining[h3_start..];

            let Some(href_start) = remaining.find("href=\"") else {
                break;
            };

            let next_h3 = remaining[3..]
                .find("<h3")
                .map(|pos| pos + 3)
                .unwrap_or(remaining.len());
            if href_start > next_h3 {
                remaining = &remaining[next_h3..];
                continue;
            }

            let href = &remaining[href_start + 6..];
            let Some(href_end) = href.find('"') else {
                break;
            };
            let url = &href[..href_end];

            let url_str = if let Some(ru_start) = url.find("/RU=") {
                let ru_part = &url[ru_start + 4..];
                let ru_end = ru_part.find('/').unwrap_or(ru_part.len());
                Self::percent_decode(&ru_part[..ru_end])
            } else {
                url.to_string()
            };

            let Some(title_start) = remaining[href_start + 6 + href_end..].find('>') else {
                break;
            };
            let anchor_content = &remaining[href_start + 6 + href_end + title_start + 1..];
            let Some(title_end) = anchor_content.find("</a>") else {
                break;
            };
            let title = Self::clean_html(&anchor_content[..title_end]);

            remaining = &anchor_content[title_end..];

            let next_result = remaining.find("<h3").unwrap_or(remaining.len());
            let result_area = &remaining[..next_result];

            let snippet = if let Some(comp_text_start) = result_area.find("compText") {
                let text_area = &result_area[comp_text_start..];
                let content_start = text_area.find('>').unwrap_or(0) + 1;
                let content_area = &text_area[content_start..];
                let content_end = content_area.find("</div>").unwrap_or(content_area.len());
                Self::clean_html(&content_area[..content_end])
            } else {
                String::new()
            };

            if !title.is_empty() && !url_str.is_empty() && !url_str.contains("search.yahoo.com/") {
                results.push(json!({
                    "title": title,
                    "url": url_str,
                    "snippet": snippet,
                }));
            }
            remaining = &remaining[next_result..];
        }

        results
    }

    fn search_wikipedia(query: &str, max_results: usize) -> Result<Vec<Value>, String> {
        let url = format!(
            "https://en.wikipedia.org/w/api.php?action=opensearch&search={}&limit={}&format=json",
            Self::percent_encode(query.as_bytes()),
            max_results
        );
        let output = Command::new("curl")
            .args([
                "-sS",
                "-L",
                "--max-time",
                "60",
                "--retry",
                "3",
                "--retry-delay",
                "2",
                "--user-agent",
                "Marix/1.0",
            ])
            .arg(url)
            .output()
            .map_err(|error| format!("failed to execute curl: {error}"))?;

        if !output.status.success() {
            return Err(format!(
                "Wikipedia request failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }

        let page = String::from_utf8_lossy(&output.stdout);
        let parsed: Value = marix_common::external::serde_json::from_str(&page)
            .map_err(|e| format!("failed to parse Wikipedia JSON: {e}"))?;

        let mut results = Vec::new();
        if let Some(arr) = parsed.as_array() {
            if arr.len() >= 4 {
                if let (Some(titles), Some(snippets), Some(urls)) =
                    (arr[1].as_array(), arr[2].as_array(), arr[3].as_array())
                {
                    let len = titles
                        .len()
                        .min(snippets.len())
                        .min(urls.len())
                        .min(max_results);
                    for i in 0..len {
                        if let (Some(title), Some(snippet), Some(url)) =
                            (titles[i].as_str(), snippets[i].as_str(), urls[i].as_str())
                        {
                            if !title.is_empty() && !url.is_empty() {
                                results.push(json!({
                                    "title": title.to_owned(),
                                    "url": url.to_owned(),
                                    "snippet": snippet.to_owned(),
                                }));
                            }
                        }
                    }
                }
            }
        }
        Ok(results)
    }

    fn result_url(href: &str) -> String {
        let href = Self::html_decode(href);
        let href = match href.strip_prefix("//") {
            Some(value) => format!("https:{value}"),
            None => href,
        };
        if let Some(query) = href.split_once("uddg=").map(|(_, query)| query) {
            let encoded = query.split('&').next().unwrap_or(query);
            return Self::percent_decode(encoded);
        }
        href
    }

    fn snippet(area: &str) -> String {
        let Some(start) = area.find("result__snippet") else {
            return String::new();
        };
        let area = &area[start..];
        let Some(content_start) = area.find('>') else {
            return String::new();
        };
        let content = &area[content_start + 1..];
        let end = content.find("</").unwrap_or(content.len());
        Self::clean_html(&content[..end])
    }

    fn clean_html(value: &str) -> String {
        let mut plain = String::with_capacity(value.len());
        let mut in_tag = false;
        for character in value.chars() {
            match character {
                '<' => in_tag = true,
                '>' => in_tag = false,
                _ if !in_tag => plain.push(character),
                _ => {}
            }
        }
        Self::html_decode(&plain)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn html_decode(value: &str) -> String {
        value
            .replace("&amp;", "&")
            .replace("&quot;", "\"")
            .replace("&#x27;", "'")
            .replace("&#39;", "'")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
    }

    fn percent_encode(bytes: &[u8]) -> String {
        let mut encoded = String::with_capacity(bytes.len());
        for byte in bytes {
            if byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'.' | b'_' | b'~') {
                encoded.push(*byte as char);
            } else {
                encoded.push('%');
                encoded.push_str(&format!("{byte:02X}"));
            }
        }
        encoded
    }

    fn percent_decode(value: &str) -> String {
        let mut decoded = Vec::with_capacity(value.len());
        let bytes = value.as_bytes();
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] == b'%' && index + 2 < bytes.len() {
                if let (Some(high), Some(low)) = (
                    Self::hex_value(bytes[index + 1]),
                    Self::hex_value(bytes[index + 2]),
                ) {
                    decoded.push(high << 4 | low);
                    index += 3;
                    continue;
                }
            }
            decoded.push(if bytes[index] == b'+' {
                b' '
            } else {
                bytes[index]
            });
            index += 1;
        }
        String::from_utf8_lossy(&decoded).into_owned()
    }

    fn hex_value(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            b'A'..=b'F' => Some(byte - b'A' + 10),
            _ => None,
        }
    }

    fn failure(message: String) -> String {
        to_string(&json!({ "error": message })).unwrap_or_default()
    }
}
