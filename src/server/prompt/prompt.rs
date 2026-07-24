use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

use marix_common::{Config, external::*};

use super::PromptError;

const PARAMETER_OPENING: &str = "{{#";
const PARAMETER_MARKER_PATTERN: &str = r"\{\{#([A-Za-z0-9_]+?)\}\}";

static PARAMETER_MARKER: OnceLock<regex::Regex> = OnceLock::new();

pub struct Prompt {
    slices: Vec<String>,
    injections: HashMap<String, Option<String>>,
}

impl Prompt {
    pub fn load(name: &str) -> Self {
        Self::assert_identifier("template", name);
        let config = Config::load()
            .unwrap_or_else(|error| panic!("failed to load config for prompt `{name}`: {error}"));
        let directory = Path::new(&config.runtime.marix_path).join("prompt");
        let path = directory.join(format!("{name}.prompt"));
        let content = Self::read(&path, "template");
        if content.contains("[[#") {
            panic!(
                "prompt template {} contains unsupported module markers",
                path.display()
            );
        }
        let (slices, injections) = Self::slice_marker(content);
        Self { slices, injections }
    }

    pub fn parameters(&self) -> Vec<String> {
        let mut parameters = Vec::new();
        for slice in &self.slices {
            if let Some(name) = Self::parameter_name(slice)
                && !parameters.iter().any(|parameter| parameter == name)
            {
                parameters.push(name.to_owned());
            }
        }
        parameters
    }

    pub fn inject(&mut self, parameter: String, value: String) {
        Self::assert_identifier("parameter", &parameter);
        if let Some(injection) = self.injections.get_mut(&parameter) {
            *injection = Some(value);
        }
    }

    pub fn prompt(&self) -> Result<String, PromptError> {
        let missing = self
            .parameters()
            .into_iter()
            .filter(|parameter| !matches!(self.injections.get(parameter), Some(Some(_))))
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(PromptError::MissingParameters(missing));
        }

        let capacity = self.slices.iter().map(String::len).sum();
        let mut prompt = String::with_capacity(capacity);
        for slice in &self.slices {
            if let Some(name) = Self::parameter_name(slice) {
                let Some(value) = self.injections.get(name).and_then(Option::as_ref) else {
                    return Err(PromptError::MissingParameters(vec![name.to_owned()]));
                };
                prompt.push_str(value);
            } else {
                prompt.push_str(slice);
            }
        }
        Ok(prompt)
    }
}

// -- Private -- //

impl Prompt {
    fn slice_marker(content: String) -> (Vec<String>, HashMap<String, Option<String>>) {
        let mut slices = Vec::new();
        let mut injections = HashMap::new();
        let mut previous_end = 0;
        for captures in Self::parameter_marker().captures_iter(&content) {
            let marker = captures
                .get(0)
                .unwrap_or_else(|| panic!("parameter marker regex did not capture a marker"));
            let name = captures
                .get(1)
                .map(|capture| capture.as_str())
                .unwrap_or_else(|| panic!("parameter marker regex did not capture a name"));
            Self::push_text_slice(
                &mut slices,
                &content[previous_end..marker.start()],
                previous_end,
            );
            slices.push(marker.as_str().to_owned());
            injections.entry(name.to_owned()).or_insert(None);
            previous_end = marker.end();
        }
        Self::push_text_slice(&mut slices, &content[previous_end..], previous_end);
        (slices, injections)
    }

    fn push_text_slice(slices: &mut Vec<String>, text: &str, offset: usize) {
        if let Some(relative_start) = text.find(PARAMETER_OPENING) {
            let start = offset + relative_start;
            panic!(
                "malformed parameter marker at byte {start}: expected \
                 `{{{{#name}}}}` with an ASCII alphanumeric or underscore name"
            );
        }
        if !text.is_empty() {
            slices.push(text.to_owned());
        }
    }

    fn parameter_name(slice: &str) -> Option<&str> {
        let captures = Self::parameter_marker().captures(slice)?;
        let marker = captures.get(0)?;
        if marker.start() == 0 && marker.end() == slice.len() {
            captures.get(1).map(|capture| capture.as_str())
        } else {
            None
        }
    }

    fn parameter_marker() -> &'static regex::Regex {
        PARAMETER_MARKER.get_or_init(|| {
            regex::Regex::new(PARAMETER_MARKER_PATTERN)
                .unwrap_or_else(|error| panic!("invalid parameter marker regex: {error}"))
        })
    }

    fn assert_identifier(kind: &str, name: &str) {
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            panic!(
                "invalid {kind} name `{name}`: expected only ASCII \
                 letters, digits, or underscore"
            );
        }
    }

    fn read(path: &Path, kind: &str) -> String {
        fs::read_to_string(path).unwrap_or_else(|error| {
            panic!("failed to read prompt {kind} {}: {error}", path.display())
        })
    }
}
