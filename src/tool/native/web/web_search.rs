mod providers;

use std::{collections::HashSet, process::Command};

use marix_common::external::serde_json::Value;

const CONNECT_TIMEOUT_SECONDS: &str = "4";
const TOTAL_TIMEOUT_SECONDS: &str = "12";
const MAX_REDIRECTS: &str = "5";
const MAX_RESPONSE_BYTES: usize = 1_048_576;
const CURL_METADATA_MARKER: &str = "__MARIX_CURL_METADATA__\t";
const USER_AGENT: &str = "Marix/1.0 (+https://github.com/DexterDreeeam/Marix)";

pub struct WebSearch;

#[cfg(feature = "web_search")]
pub use self::WebSearch as SelectedTool;

// -- Private -- //

#[derive(Clone, Copy)]
enum Source {
    BingRss,
    DuckDuckGo,
    Yahoo,
    Wikipedia,
}

impl Source {
    fn name(self) -> &'static str {
        match self {
            Self::BingRss => "Bing RSS",
            Self::DuckDuckGo => "DuckDuckGo",
            Self::Yahoo => "Yahoo",
            Self::Wikipedia => "Wikipedia",
        }
    }

    fn accept(self) -> &'static str {
        match self {
            Self::BingRss => "application/rss+xml, application/xml;q=0.9, text/xml;q=0.8",
            Self::DuckDuckGo | Self::Yahoo => "text/html, application/xhtml+xml;q=0.9",
            Self::Wikipedia => "application/json",
        }
    }

    fn accepts_content_type(self, content_type: &str) -> bool {
        let essence = content_type
            .split(';')
            .next()
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        match self {
            Self::BingRss => {
                essence == "text/xml"
                    || essence == "application/xml"
                    || essence == "application/rss+xml"
                    || essence.ends_with("+xml")
            }
            Self::DuckDuckGo | Self::Yahoo => {
                essence == "text/html" || essence == "application/xhtml+xml"
            }
            Self::Wikipedia => essence == "application/json",
        }
    }
}

struct SearchResult {
    title: String,
    url: String,
    snippet: String,
}

impl WebSearch {
    const NAME: &'static str = "web_search";
    const DESCRIPTION: &'static str =
        "Search the web and return result titles, URLs, and snippets.";
    const INPUT_SCHEMA: &'static str = r#"{"type":"object","properties":{"query":{"type":"string","minLength":1},"max_results":{"type":"integer","minimum":1,"maximum":10}},"required":["query"],"additionalProperties":false}"#;
    const MAX_RESULTS: usize = 10;

    fn request(source: Source, url: String) -> Result<String, String> {
        let accept = format!("Accept: {}", source.accept());
        let write_out = format!(
            "%{{stderr}}\n{CURL_METADATA_MARKER}%{{http_code}}\t\
             %{{content_type}}\t%{{size_download}}\n"
        );
        let output = Command::new(Self::curl_program())
            .args([
                "--silent",
                "--show-error",
                "--location",
                "--proto",
                "=https",
                "--proto-redir",
                "=https",
                "--connect-timeout",
                CONNECT_TIMEOUT_SECONDS,
                "--max-time",
                TOTAL_TIMEOUT_SECONDS,
                "--max-redirs",
                MAX_REDIRECTS,
                "--max-filesize",
                "1048576",
                "--ipv4",
                "--user-agent",
                USER_AGENT,
                "--header",
                &accept,
                "--write-out",
                &write_out,
            ])
            .arg(url)
            .output()
            .map_err(|error| format!("failed to execute curl: {error}"))?;

        let stderr = String::from_utf8_lossy(&output.stderr);
        let metadata = Self::curl_metadata(&stderr);
        if !output.status.success() {
            return Err(format!(
                "request failed: {}",
                Self::curl_error(&stderr, output.status.code())
            ));
        }
        let (status, content_type, downloaded) = metadata?;
        if !(200..300).contains(&status) {
            return Err(format!("HTTP status {status}"));
        }
        if !source.accepts_content_type(&content_type) {
            return Err(format!(
                "unexpected Content-Type {}",
                if content_type.is_empty() {
                    "<empty>"
                } else {
                    &content_type
                }
            ));
        }
        if downloaded > MAX_RESPONSE_BYTES as f64 || output.stdout.len() > MAX_RESPONSE_BYTES {
            return Err(format!("response exceeded {MAX_RESPONSE_BYTES} bytes"));
        }
        if output.stdout.is_empty() {
            return Err("response body was empty".to_owned());
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    fn curl_metadata(stderr: &str) -> Result<(u16, String, f64), String> {
        let Some(marker) = stderr.rfind(CURL_METADATA_MARKER) else {
            return Err("curl did not provide HTTP metadata".to_owned());
        };
        let line = stderr[marker + CURL_METADATA_MARKER.len()..]
            .lines()
            .next()
            .unwrap_or_default();
        let mut fields = line.splitn(3, '\t');
        let status = fields
            .next()
            .and_then(|value| value.parse::<u16>().ok())
            .ok_or_else(|| "curl returned an invalid HTTP status".to_owned())?;
        let content_type = fields.next().unwrap_or_default().trim().to_owned();
        let downloaded = fields
            .next()
            .and_then(|value| value.parse::<f64>().ok())
            .ok_or_else(|| "curl returned an invalid response size".to_owned())?;
        Ok((status, content_type, downloaded))
    }

    fn curl_error(stderr: &str, exit_code: Option<i32>) -> String {
        let diagnostics = stderr
            .split(CURL_METADATA_MARKER)
            .next()
            .unwrap_or(stderr)
            .trim();
        let detail = diagnostics
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .unwrap_or_default()
            .trim();
        if detail.is_empty() {
            return exit_code
                .map(|code| format!("curl exited with code {code}"))
                .unwrap_or_else(|| "curl was terminated".to_owned());
        }
        detail.chars().take(240).collect()
    }

    #[cfg(windows)]
    fn curl_program() -> &'static str {
        "curl.exe"
    }

    #[cfg(not(windows))]
    fn curl_program() -> &'static str {
        "curl"
    }

    fn merge(
        outcomes: Vec<(Source, Result<Vec<SearchResult>, String>)>,
        max_results: usize,
    ) -> Result<Vec<Value>, String> {
        let mut successful = Vec::new();
        let mut reasons = Vec::new();
        for (source, outcome) in outcomes {
            match outcome {
                Ok(results) if !results.is_empty() => {
                    successful.push(results);
                }
                Ok(_) => reasons.push(format!("{}: returned no valid results", source.name())),
                Err(error) => {
                    reasons.push(format!("{}: {error}", source.name()));
                }
            }
        }
        if successful.is_empty() {
            return Err(format!(
                "All search engines failed. Errors: {}",
                reasons.join("; ")
            ));
        }

        let mut merged = Vec::new();
        let mut seen = HashSet::new();
        for position in 0..Self::MAX_RESULTS {
            for results in &successful {
                let Some(result) = results.get(position) else {
                    continue;
                };
                let Some(key) = Self::normalized_url(&result.url) else {
                    continue;
                };
                if seen.insert(key) {
                    merged.push(marix_common::external::serde_json::json!({
                        "title": result.title.as_str(),
                        "url": result.url.as_str(),
                        "snippet": result.snippet.as_str(),
                    }));
                }
                if merged.len() == max_results {
                    return Ok(merged);
                }
            }
        }
        Ok(merged)
    }

    fn normalized_url(value: &str) -> Option<String> {
        let without_fragment = value.trim().split('#').next()?.trim();
        let (scheme, remainder) = without_fragment.split_once("://")?;
        let scheme = scheme.to_ascii_lowercase();
        if scheme != "http" && scheme != "https" {
            return None;
        }
        let authority_end = remainder.find(['/', '?']).unwrap_or(remainder.len());
        let authority = &remainder[..authority_end];
        if authority.is_empty() {
            return None;
        }
        let tail = &remainder[authority_end..];
        let (userinfo, host_port) = authority
            .rsplit_once('@')
            .map(|(userinfo, host)| (Some(userinfo), host))
            .unwrap_or((None, authority));
        let (host, port) = Self::host_and_port(host_port)?;
        let mut normalized = format!("{scheme}://");
        if let Some(userinfo) = userinfo {
            normalized.push_str(userinfo);
            normalized.push('@');
        }
        normalized.push_str(&host.to_ascii_lowercase());
        if port.is_some_and(|port| {
            !((scheme == "http" && port == "80") || (scheme == "https" && port == "443"))
        }) {
            normalized.push(':');
            normalized.push_str(port.unwrap_or_default());
        }

        let (path, query) = tail
            .split_once('?')
            .map(|(path, query)| (path, Some(query)))
            .unwrap_or((tail, None));
        normalized.push_str(if path.is_empty() { "/" } else { path });
        if let Some(query) = query {
            let retained = query
                .split('&')
                .filter(|parameter| {
                    let key = parameter
                        .split('=')
                        .next()
                        .unwrap_or_default()
                        .to_ascii_lowercase();
                    !key.starts_with("utm_")
                        && !matches!(key.as_str(), "fbclid" | "gclid" | "msclkid")
                })
                .collect::<Vec<_>>();
            if !retained.is_empty() {
                normalized.push('?');
                normalized.push_str(&retained.join("&"));
            }
        }
        Some(normalized)
    }

    fn host_and_port(authority: &str) -> Option<(&str, Option<&str>)> {
        if authority.starts_with('[') {
            let end = authority.find(']')?;
            let host = &authority[..=end];
            let suffix = &authority[end + 1..];
            return match suffix {
                "" => Some((host, None)),
                value if value.starts_with(':') => {
                    let port = &value[1..];
                    (!port.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit()))
                        .then_some((host, Some(port)))
                }
                _ => None,
            };
        }
        match authority.rsplit_once(':') {
            Some((host, port))
                if !host.is_empty()
                    && !host.contains(':')
                    && !port.is_empty()
                    && port.bytes().all(|byte| byte.is_ascii_digit()) =>
            {
                Some((host, Some(port)))
            }
            Some(_) => None,
            _ if !authority.is_empty() => Some((authority, None)),
            _ => None,
        }
    }

    fn finish_source(
        page: &str,
        mut results: Vec<SearchResult>,
    ) -> Result<Vec<SearchResult>, String> {
        if Self::looks_like_challenge(page) {
            return Err("challenge page detected".to_owned());
        }
        results.retain(|result| {
            !result.title.trim().is_empty() && Self::normalized_url(&result.url).is_some()
        });
        if results.is_empty() {
            return Err("returned no valid results".to_owned());
        }
        Ok(results)
    }

    fn looks_like_challenge(page: &str) -> bool {
        let lower = page.to_ascii_lowercase();
        [
            "<title>just a moment",
            "<title>captcha",
            "id=\"challenge-form\"",
            "cf-chl-",
            "verify you are human",
            "systems have detected unusual traffic",
        ]
        .iter()
        .any(|marker| lower.contains(marker))
    }
}
