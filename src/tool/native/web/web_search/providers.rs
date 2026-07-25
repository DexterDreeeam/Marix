use marix_common::{
    Arch, Platform, System,
    external::serde_json::{Value, json, to_string},
};
use marix_protocol::{ToolCategory, ToolPreview};
use roxmltree::{Document, Node};

use super::{SearchResult, Source, WebSearch};
use crate::{ToolProgram, native::parse_input};

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
            Err(error) => {
                return Self::failure(format!("invalid input: {error}"));
            }
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

// -- Private -- //

impl WebSearch {
    fn search(query: &str, max_results: usize) -> Result<Vec<Value>, String> {
        std::thread::scope(|scope| {
            let workers = [
                (
                    Source::BingRss,
                    scope.spawn(|| Self::search_bing_rss(query)),
                ),
                (
                    Source::DuckDuckGo,
                    scope.spawn(|| Self::search_duckduckgo(query)),
                ),
                (Source::Yahoo, scope.spawn(|| Self::search_yahoo(query))),
                (
                    Source::Wikipedia,
                    scope.spawn(|| Self::search_wikipedia(query)),
                ),
            ];
            let outcomes = workers
                .into_iter()
                .map(|(source, worker)| {
                    let result = worker
                        .join()
                        .unwrap_or_else(|_| Err("worker thread panicked".to_owned()));
                    (source, result)
                })
                .collect();
            Self::merge(outcomes, max_results)
        })
    }

    fn search_bing_rss(query: &str) -> Result<Vec<SearchResult>, String> {
        let url = format!(
            "https://www.bing.com/search?format=rss&q={}",
            Self::percent_encode(query.as_bytes())
        );
        let page = Self::request(Source::BingRss, url)?;
        let document =
            Document::parse(&page).map_err(|error| format!("RSS parse failed: {error}"))?;
        let results = document
            .descendants()
            .filter(|node| node.is_element() && node.tag_name().name().eq_ignore_ascii_case("item"))
            .take(Self::MAX_RESULTS)
            .filter_map(|item| {
                let title = Self::rss_text(item, "title");
                let url = Self::rss_text(item, "link");
                if title.trim().is_empty() || url.trim().is_empty() {
                    return None;
                }
                Some(SearchResult {
                    title: Self::clean_html(&title),
                    url: url.trim().to_owned(),
                    snippet: Self::clean_html(&Self::rss_text(item, "description")),
                })
            })
            .collect();
        Self::finish_source(&page, results)
    }

    fn rss_text(item: Node<'_, '_>, name: &str) -> String {
        item.children()
            .find(|child| child.is_element() && child.tag_name().name().eq_ignore_ascii_case(name))
            .map(|element| {
                element
                    .descendants()
                    .filter(|node| node.is_text())
                    .filter_map(|node| node.text())
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default()
    }

    fn search_duckduckgo(query: &str) -> Result<Vec<SearchResult>, String> {
        let url = format!(
            "https://html.duckduckgo.com/html/?q={}",
            Self::percent_encode(query.as_bytes())
        );
        let page = Self::request(Source::DuckDuckGo, url)?;
        let results = Self::parse_duckduckgo_results(&page);
        Self::finish_source(&page, results)
    }

    fn parse_duckduckgo_results(page: &str) -> Vec<SearchResult> {
        let mut results = Vec::new();
        let mut remaining = page;
        while results.len() < Self::MAX_RESULTS {
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
            let snippet = Self::snippet(&after_anchor[..next_result]);
            if !title.is_empty() && !url.is_empty() {
                results.push(SearchResult {
                    title,
                    url,
                    snippet,
                });
            }
            remaining = &after_anchor[next_result..];
        }
        results
    }

    fn search_yahoo(query: &str) -> Result<Vec<SearchResult>, String> {
        let url = format!(
            "https://search.yahoo.com/search?p={}",
            Self::percent_encode(query.as_bytes())
        );
        let page = Self::request(Source::Yahoo, url)?;
        let results = Self::parse_yahoo_results(&page);
        Self::finish_source(&page, results)
    }

    fn parse_yahoo_results(page: &str) -> Vec<SearchResult> {
        let mut results = Vec::new();
        let mut remaining = page;
        while results.len() < Self::MAX_RESULTS {
            let Some(h3_start) = remaining.find("<h3") else {
                break;
            };
            remaining = &remaining[h3_start..];
            let Some(href_start) = remaining.find("href=\"") else {
                break;
            };
            let next_h3 = remaining[3..]
                .find("<h3")
                .map(|position| position + 3)
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
            let url = if let Some(ru_start) = url.find("/RU=") {
                let encoded = &url[ru_start + 4..];
                let end = encoded.find('/').unwrap_or(encoded.len());
                Self::percent_decode(&encoded[..end])
            } else {
                url.to_owned()
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
            let snippet = result_area
                .find("compText")
                .map(|start| &result_area[start..])
                .and_then(|area| area.find('>').map(|start| &area[start + 1..]))
                .map(|area| &area[..area.find("</div>").unwrap_or(area.len())])
                .map(Self::clean_html)
                .unwrap_or_default();
            if !title.is_empty() && !url.is_empty() && !url.contains("search.yahoo.com/") {
                results.push(SearchResult {
                    title,
                    url,
                    snippet,
                });
            }
            remaining = &remaining[next_result..];
        }
        results
    }

    fn search_wikipedia(query: &str) -> Result<Vec<SearchResult>, String> {
        let url = format!(
            "https://en.wikipedia.org/w/api.php?action=opensearch&\
             search={}&limit={}&format=json",
            Self::percent_encode(query.as_bytes()),
            Self::MAX_RESULTS
        );
        let page = Self::request(Source::Wikipedia, url)?;
        let parsed: Value = marix_common::external::serde_json::from_str(&page)
            .map_err(|error| format!("JSON parse failed: {error}"))?;
        let mut results = Vec::new();
        if let Some(items) = parsed.as_array()
            && items.len() >= 4
            && let (Some(titles), Some(snippets), Some(urls)) = (
                items[1].as_array(),
                items[2].as_array(),
                items[3].as_array(),
            )
        {
            let count = titles
                .len()
                .min(snippets.len())
                .min(urls.len())
                .min(Self::MAX_RESULTS);
            for index in 0..count {
                if let (Some(title), Some(snippet), Some(url)) = (
                    titles[index].as_str(),
                    snippets[index].as_str(),
                    urls[index].as_str(),
                ) {
                    results.push(SearchResult {
                        title: title.to_owned(),
                        url: url.to_owned(),
                        snippet: snippet.to_owned(),
                    });
                }
            }
        }
        Self::finish_source(&page, results)
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
        let decoded = Self::html_decode(value);
        let mut plain = String::with_capacity(decoded.len());
        let mut remaining = decoded.as_str();
        while let Some(start) = remaining.find('<') {
            plain.push_str(&remaining[..start]);
            let after_open = &remaining[start + 1..];
            let Some(end) = after_open.find('>') else {
                plain.push_str(&remaining[start..]);
                remaining = "";
                break;
            };
            let first = after_open.as_bytes().first().copied();
            if first.is_some_and(|byte| {
                byte.is_ascii_alphabetic() || matches!(byte, b'/' | b'!' | b'?')
            }) {
                remaining = &after_open[end + 1..];
            } else {
                plain.push('<');
                remaining = after_open;
            }
        }
        plain.push_str(remaining);
        Self::html_decode(&plain)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn html_decode(value: &str) -> String {
        let mut decoded = String::with_capacity(value.len());
        let mut remaining = value;
        while let Some(start) = remaining.find('&') {
            decoded.push_str(&remaining[..start]);
            remaining = &remaining[start..];
            let Some(end) = remaining.find(';').filter(|end| *end <= 12) else {
                decoded.push('&');
                remaining = &remaining[1..];
                continue;
            };
            let entity = &remaining[1..end];
            if let Some(character) = Self::entity_character(entity) {
                decoded.push(character);
                remaining = &remaining[end + 1..];
            } else {
                decoded.push('&');
                remaining = &remaining[1..];
            }
        }
        decoded.push_str(remaining);
        decoded
    }

    fn entity_character(entity: &str) -> Option<char> {
        match entity {
            "amp" => Some('&'),
            "quot" => Some('"'),
            "apos" | "#39" | "#x27" => Some('\''),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "nbsp" => Some(' '),
            _ => entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
                .and_then(|value| u32::from_str_radix(value, 16).ok())
                .or_else(|| {
                    entity
                        .strip_prefix('#')
                        .and_then(|value| value.parse::<u32>().ok())
                })
                .and_then(char::from_u32),
        }
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
