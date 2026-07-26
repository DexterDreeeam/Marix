use std::collections::HashSet;
use std::process::Command;

use marix_common::external::reqwest::Url;

const MAX_REDIRECTS: usize = 10;
const CURL_METADATA_MARKER: &str =
    "__MARIX_WEB_FETCH_CURL_METADATA__\t";

struct CurlMetadata {
    status: u16,
    content_type: String,
    effective_url: String,
    redirects: usize,
}

struct CurlResponse {
    body: String,
    metadata: CurlMetadata,
    diagnostics: String,
}

pub(super) fn fetch(url: &str) -> Result<(String, String), String> {
    let mut current = parse_url(url)?;
    let mut seen = HashSet::new();
    seen.insert(canonical_url(&current));
    let mut redirect_count = 0;

    loop {
        let remaining = MAX_REDIRECTS.saturating_sub(redirect_count);
        let response = request(current.as_str(), remaining)?;
        redirect_count = redirect_count
            .checked_add(response.metadata.redirects)
            .ok_or_else(redirect_limit_error)?;
        if redirect_count > MAX_REDIRECTS {
            return Err(redirect_limit_error());
        }

        let effective = parse_url(&response.metadata.effective_url)?;
        let current_key = canonical_url(&current);
        let effective_key = canonical_url(&effective);
        if response.metadata.redirects > 0 && effective_key == current_key {
            return Err(format!(
                "redirect loop detected at {effective_key}"
            ));
        }
        if effective_key != current_key
            && !seen.insert(effective_key.clone())
        {
            return Err(format!(
                "redirect loop detected at {effective_key}"
            ));
        }
        if !(200..300).contains(&response.metadata.status) {
            let diagnostics = if response.diagnostics.is_empty() {
                "<empty>"
            } else {
                &response.diagnostics
            };
            return Err(format!(
                "HTTP status {} for {}; curl stderr: {}",
                response.metadata.status, effective_key, diagnostics,
            ));
        }

        if !is_html(
            &response.metadata.content_type,
            &response.body,
        ) {
            return Ok((effective_key, response.body));
        }
        let Some(target) = html_redirect(&response.body)? else {
            return Ok((effective_key, response.body));
        };

        redirect_count += 1;
        if redirect_count > MAX_REDIRECTS {
            return Err(redirect_limit_error());
        }
        let next = effective
            .join(&target)
            .map_err(|error| {
                format!(
                    "invalid HTML redirect URL `{target}` from \
                     {effective_key}: {error}"
                )
            })
            .and_then(validate_url)?;
        let next_key = canonical_url(&next);
        if !seen.insert(next_key.clone()) {
            return Err(format!("redirect loop detected at {next_key}"));
        }
        current = next;
    }
}

fn request(url: &str, max_redirects: usize) -> Result<CurlResponse, String> {
    let max_redirects_arg = max_redirects.to_string();
    let write_out = format!(
        "%{{stderr}}\n{CURL_METADATA_MARKER}%{{http_code}}\t\
         %{{content_type}}\t%{{url_effective}}\t%{{num_redirects}}\n"
    );
    let output = Command::new("curl")
        .args([
            "--silent",
            "--show-error",
            "--location",
            "--proto",
            "=http,https",
            "--proto-redir",
            "=http,https",
            "--max-time",
            "60",
            "--retry",
            "3",
            "--retry-delay",
            "2",
            "--max-redirs",
            &max_redirects_arg,
            "--write-out",
            &write_out,
        ])
        .arg(url)
        .output()
        .map_err(|error| format!("failed to execute curl: {error}"))?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    let diagnostics = curl_diagnostics(&stderr);
    let metadata = curl_metadata(&stderr);
    if !output.status.success() {
        if output.status.code() == Some(47) {
            return Err(redirect_limit_error());
        }
        let detail = if diagnostics.is_empty() {
            output
                .status
                .code()
                .map(|code| format!("curl exited with code {code}"))
                .unwrap_or_else(|| "curl was terminated".to_owned())
        } else {
            diagnostics
        };
        if metadata.as_ref().is_ok_and(|value| {
            max_redirects == 0 && (300..400).contains(&value.status)
        }) {
            return Err(redirect_limit_error());
        }
        return Err(format!("curl request failed for {url}: {detail}"));
    }

    let metadata = metadata?;
    Ok(CurlResponse {
        body: String::from_utf8_lossy(&output.stdout).into_owned(),
        metadata,
        diagnostics,
    })
}

fn curl_metadata(stderr: &str) -> Result<CurlMetadata, String> {
    let Some(marker) = stderr.rfind(CURL_METADATA_MARKER) else {
        return Err("curl did not provide HTTP metadata".to_owned());
    };
    let line = stderr[marker + CURL_METADATA_MARKER.len()..]
        .lines()
        .next()
        .unwrap_or_default();
    let mut fields = line.splitn(4, '\t');
    let status = fields
        .next()
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| {
            "curl returned an invalid HTTP status".to_owned()
        })?;
    let content_type = fields.next().unwrap_or_default().trim().to_owned();
    let effective_url = fields.next().unwrap_or_default().trim().to_owned();
    if effective_url.is_empty() {
        return Err("curl returned an empty effective URL".to_owned());
    }
    let redirects = fields
        .next()
        .and_then(|value| value.trim().parse().ok())
        .ok_or_else(|| {
            "curl returned an invalid redirect count".to_owned()
        })?;
    Ok(CurlMetadata {
        status,
        content_type,
        effective_url,
        redirects,
    })
}

fn curl_diagnostics(stderr: &str) -> String {
    stderr
        .split(CURL_METADATA_MARKER)
        .next()
        .unwrap_or(stderr)
        .trim()
        .to_owned()
}

fn parse_url(url: &str) -> Result<Url, String> {
    Url::parse(url)
        .map_err(|error| format!("invalid URL `{url}`: {error}"))
        .and_then(validate_url)
}

fn validate_url(mut url: Url) -> Result<Url, String> {
    if !matches!(url.scheme(), "http" | "https") {
        return Err(format!(
            "unsupported URL scheme `{}`; only http and https are allowed",
            url.scheme(),
        ));
    }
    url.set_fragment(None);
    Ok(url)
}

fn canonical_url(url: &Url) -> String {
    url.as_str().to_owned()
}

fn redirect_limit_error() -> String {
    format!("redirect limit exceeded (maximum {MAX_REDIRECTS})")
}

fn is_html(content_type: &str, body: &str) -> bool {
    let content_type = content_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim();
    if content_type.eq_ignore_ascii_case("text/html")
        || content_type.eq_ignore_ascii_case("application/xhtml+xml")
    {
        return true;
    }
    let body = body.to_ascii_lowercase();
    body.contains("<!doctype html")
        || body.contains("<html")
        || body.contains("<meta")
        || body.contains("<link")
}

fn html_redirect(html: &str) -> Result<Option<String>, String> {
    let mut offset = 0;
    let mut refresh_seen = false;
    let mut refresh_target = None;
    let mut canonical_target = None;

    while let Some(relative_start) = html[offset..].find('<') {
        let start = offset + relative_start;
        let Some(end) = find_tag_end(html, start + 1) else {
            break;
        };
        let tag = &html[start + 1..end];
        let name = tag_name(tag);
        if name.is_some_and(|name| name.eq_ignore_ascii_case("meta"))
            && attribute(tag, "http-equiv")
                .is_some_and(|value| {
                    value.trim().eq_ignore_ascii_case("refresh")
                })
        {
            refresh_seen = true;
            if refresh_target.is_none() {
                refresh_target = attribute(tag, "content")
                    .and_then(refresh_url);
            }
        } else if name
            .is_some_and(|name| name.eq_ignore_ascii_case("link"))
            && attribute(tag, "rel").is_some_and(|value| {
                value
                    .split_ascii_whitespace()
                    .any(|part| part.eq_ignore_ascii_case("canonical"))
            })
            && canonical_target.is_none()
        {
            canonical_target = attribute(tag, "href")
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned);
        }
        offset = end + 1;
    }

    if !refresh_seen {
        return Ok(None);
    }
    refresh_target
        .or(canonical_target)
        .map(Some)
        .ok_or_else(|| {
            "HTML meta refresh did not provide a redirect URL".to_owned()
        })
}

fn refresh_url(content: &str) -> Option<String> {
    let bytes = content.as_bytes();
    let mut offset = 0;
    while offset + 3 <= bytes.len() {
        let at_boundary = offset == 0
            || bytes[offset - 1].is_ascii_whitespace()
            || bytes[offset - 1] == b';';
        if at_boundary
            && bytes[offset..offset + 3].eq_ignore_ascii_case(b"url")
        {
            let mut equals = offset + 3;
            while bytes
                .get(equals)
                .is_some_and(u8::is_ascii_whitespace)
            {
                equals += 1;
            }
            if bytes.get(equals) == Some(&b'=') {
                let value = content[equals + 1..].trim_matches(
                    |character: char| {
                        character.is_ascii_whitespace()
                            || matches!(character, '\'' | '"')
                    },
                );
                return (!value.is_empty()).then(|| value.to_owned());
            }
        }
        offset += content[offset..]
            .chars()
            .next()
            .map(char::len_utf8)
            .unwrap_or(1);
    }
    None
}

fn find_tag_end(content: &str, start: usize) -> Option<usize> {
    let mut quote = None;
    for (relative_index, byte) in
        content.as_bytes()[start..].iter().enumerate()
    {
        match (*byte, quote) {
            (b'\'' | b'"', None) => quote = Some(*byte),
            (current, Some(expected)) if current == expected => {
                quote = None;
            }
            (b'>', None) => return Some(start + relative_index),
            _ => {}
        }
    }
    None
}

fn tag_name(tag: &str) -> Option<&str> {
    let tag = tag.trim_start();
    if tag.starts_with(['/', '!', '?']) {
        return None;
    }
    let end = tag
        .find(|character: char| {
            !character.is_ascii_alphanumeric() && character != '-'
        })
        .unwrap_or(tag.len());
    (end > 0).then_some(&tag[..end])
}

fn attribute<'a>(tag: &'a str, expected: &str) -> Option<&'a str> {
    let bytes = tag.as_bytes();
    let leading_whitespace = tag.len() - tag.trim_start().len();
    let mut offset = leading_whitespace + tag_name(tag)?.len();

    while offset < bytes.len() {
        while bytes
            .get(offset)
            .is_some_and(u8::is_ascii_whitespace)
        {
            offset += 1;
        }
        if bytes.get(offset) == Some(&b'/') {
            break;
        }

        let name_start = offset;
        while bytes.get(offset).is_some_and(|byte| {
            !byte.is_ascii_whitespace()
                && !matches!(byte, b'=' | b'/' | b'>')
        }) {
            offset += 1;
        }
        if name_start == offset {
            offset += 1;
            continue;
        }
        let name = &tag[name_start..offset];
        while bytes
            .get(offset)
            .is_some_and(u8::is_ascii_whitespace)
        {
            offset += 1;
        }
        if bytes.get(offset) != Some(&b'=') {
            continue;
        }
        offset += 1;
        while bytes
            .get(offset)
            .is_some_and(u8::is_ascii_whitespace)
        {
            offset += 1;
        }

        let (value_start, value_end) = match bytes.get(offset) {
            Some(quote @ (b'\'' | b'"')) => {
                offset += 1;
                let start = offset;
                while bytes
                    .get(offset)
                    .is_some_and(|byte| byte != quote)
                {
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
                    .is_some_and(|byte| !byte.is_ascii_whitespace())
                {
                    offset += 1;
                }
                (start, offset)
            }
            None => return None,
        };
        if name.eq_ignore_ascii_case(expected) {
            return Some(&tag[value_start..value_end]);
        }
    }
    None
}
