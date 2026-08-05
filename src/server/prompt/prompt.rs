use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

use marix_common::{Config, external::*};

use super::PromptError;
use crate::stage::PromptParameter;

const MARKER_OPENING: &str = "{{";
const MARKER_PATTERN: &str =
    r"\{\{(\^?)([#@])([A-Za-z0-9_]+?)\}\}";

static MARKER: OnceLock<regex::Regex> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PromptMarker {
    Parameter {
        name: String,
        repeatable: bool,
    },
    Module {
        name: String,
        repeatable: bool,
    },
}

#[derive(Clone)]
pub struct Prompt {
    slices: Vec<PromptSlice>,
    parameters: Vec<PromptParameter>,
    injections: HashMap<String, Vec<String>>,
}

impl Prompt {
    pub fn load(name: &str) -> Prompt {
        Self::load_from(name, PromptKind::Template)
    }

    pub fn load_module(name: &str) -> Prompt {
        Self::load_from(name, PromptKind::Module)
    }

    pub fn parameters(&self) -> Vec<PromptParameter> {
        self.parameters.clone()
    }

    pub fn inject(&mut self, parameter: String, value: String) {
        Self::assert_identifier("parameter", &parameter);
        if let Some(injections) = self.injections.get_mut(&parameter) {
            injections.push(value);
        }
    }

    pub fn prompt(&self) -> Result<String, PromptError> {
        self.render()
    }
}

// -- Private -- //

#[derive(Clone)]
enum PromptSlice {
    Text(String),
    Parameter {
        name: String,
        repeatable: bool,
    },
    Module {
        name: String,
        repeatable: bool,
        prompt: Box<Prompt>,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PromptKind {
    Template,
    Module,
}

impl Prompt {
    fn load_from(name: &str, kind: PromptKind) -> Self {
        Self::assert_identifier("prompt", name);
        let config = Config::load().unwrap_or_else(|error| {
            panic!("failed to load config for prompt `{name}`: {error}")
        });
        let subdirectory = match kind {
            PromptKind::Template => "template",
            PromptKind::Module => "module",
        };
        let path = Path::new(&config.runtime.marix_path)
            .join("prompt")
            .join(subdirectory)
            .join(format!("{name}.prompt"));
        let content = Self::read(&path);
        Self::parse(content, kind, &path)
    }

    fn parse(content: String, kind: PromptKind, path: &Path) -> Self {
        let mut slices = Vec::new();
        let mut parameters = Vec::new();
        let mut injections = HashMap::new();
        let mut owners = HashMap::<String, String>::new();
        let mut previous_end = 0;
        for captures in Self::marker().captures_iter(&content) {
            let marker = captures
                .get(0)
                .unwrap_or_else(|| panic!("prompt marker regex missed marker"));
            Self::push_text_slice(
                &mut slices,
                &content[previous_end..marker.start()],
                previous_end,
            );
            let repeatable = captures
                .get(1)
                .is_some_and(|capture| capture.as_str() == "^");
            let marker_kind = captures
                .get(2)
                .map(|capture| capture.as_str())
                .unwrap_or_else(|| panic!("prompt marker regex missed kind"));
            let name = captures
                .get(3)
                .map(|capture| capture.as_str().to_owned())
                .unwrap_or_else(|| panic!("prompt marker regex missed name"));
            let prompt_marker = match marker_kind {
                "#" => PromptMarker::Parameter {
                    name,
                    repeatable,
                },
                "@" => PromptMarker::Module {
                    name,
                    repeatable,
                },
                _ => unreachable!("prompt marker regex accepted kind"),
            };
            Self::push_marker(
                &mut slices,
                &mut parameters,
                &mut injections,
                &mut owners,
                prompt_marker,
                kind,
                path,
            );
            previous_end = marker.end();
        }
        Self::push_text_slice(
            &mut slices,
            &content[previous_end..],
            previous_end,
        );
        Self {
            slices,
            parameters,
            injections,
        }
    }

    fn push_marker(
        slices: &mut Vec<PromptSlice>,
        parameters: &mut Vec<PromptParameter>,
        injections: &mut HashMap<String, Vec<String>>,
        owners: &mut HashMap<String, String>,
        marker: PromptMarker,
        kind: PromptKind,
        path: &Path,
    ) {
        match marker {
            PromptMarker::Parameter {
                name,
                repeatable,
            } => {
                Self::register_parameter(
                    parameters,
                    injections,
                    owners,
                    PromptParameter {
                        name: name.clone(),
                        required: false,
                        repeatable,
                        pack_tag: if repeatable {
                            name.clone()
                        } else {
                            String::new()
                        },
                    },
                    "template",
                );
                slices.push(PromptSlice::Parameter {
                    name,
                    repeatable,
                });
            }
            PromptMarker::Module {
                name,
                repeatable,
            } => {
                if matches!(kind, PromptKind::Module) {
                    panic!(
                        "prompt module {} cannot contain module marker \
                         `{{{{{}@{name}}}}}`",
                        path.display(),
                        if repeatable { "^" } else { "" },
                    );
                }
                let module_name = Self::module_file_name(&name);
                let module = Self::load_module(&module_name);
                if repeatable
                    && module
                        .parameters
                        .iter()
                        .any(|parameter| !parameter.repeatable)
                {
                    panic!(
                        "repeatable prompt module `{name}` contains a \
                         non-repeatable parameter"
                    );
                }
                for mut parameter in module.parameters.clone() {
                    if repeatable {
                        parameter.repeatable = true;
                        parameter.pack_tag = name.clone();
                    }
                    Self::register_parameter(
                        parameters,
                        injections,
                        owners,
                        parameter,
                        &format!("module `{name}`"),
                    );
                }
                slices.push(PromptSlice::Module {
                    name,
                    repeatable,
                    prompt: Box::new(module),
                });
            }
        }
    }

    fn register_parameter(
        parameters: &mut Vec<PromptParameter>,
        injections: &mut HashMap<String, Vec<String>>,
        owners: &mut HashMap<String, String>,
        parameter: PromptParameter,
        owner: &str,
    ) {
        if let Some(previous_owner) = owners.get(&parameter.name) {
            if previous_owner != owner {
                panic!(
                    "prompt parameter `{}` is owned by both {} and {}",
                    parameter.name, previous_owner, owner,
                );
            }
            return;
        }
        owners.insert(parameter.name.clone(), owner.to_owned());
        injections.insert(parameter.name.clone(), Vec::new());
        parameters.push(parameter);
    }

    fn render(&self) -> Result<String, PromptError> {
        let capacity = self
            .slices
            .iter()
            .map(|slice| match slice {
                PromptSlice::Text(text) => text.len(),
                _ => 0,
            })
            .sum();
        let mut output = String::with_capacity(capacity);
        for slice in &self.slices {
            match slice {
                PromptSlice::Text(text) => output.push_str(text),
                PromptSlice::Parameter {
                    name,
                    repeatable,
                } => {
                    let values = &self.injections[name];
                    if *repeatable {
                        for value in values {
                            output.push_str(value);
                        }
                    } else if let Some(value) = values.first() {
                        output.push_str(value);
                    }
                }
                PromptSlice::Module {
                    name,
                    repeatable,
                    prompt,
                } => {
                    self.render_module(
                        &mut output,
                        name,
                        *repeatable,
                        prompt,
                    )?;
                }
            }
        }
        Ok(output)
    }

    fn render_module(
        &self,
        output: &mut String,
        name: &str,
        repeatable: bool,
        module: &Prompt,
    ) -> Result<(), PromptError> {
        if !repeatable {
            let mut instance = module.clone();
            for parameter in &module.parameters {
                let Some(values) =
                    self.injections.get(&parameter.name)
                else {
                    continue;
                };
                if parameter.repeatable {
                    for value in values {
                        instance.inject(
                            parameter.name.clone(),
                            value.clone(),
                        );
                    }
                } else if let Some(value) = values.first() {
                    instance.inject(
                        parameter.name.clone(),
                        value.clone(),
                    );
                }
            }
            let _ = name;
            output.push_str(&instance.render()?);
            return Ok(());
        }
        let instance_count = module
            .parameters
            .iter()
            .map(|parameter| {
                self.injections
                    .get(&parameter.name)
                    .map(Vec::len)
                    .unwrap_or_default()
            })
            .max()
            .unwrap_or_default();
        for index in 0..instance_count {
            let mut instance = module.clone();
            for parameter in &module.parameters {
                if let Some(value) = self
                    .injections
                    .get(&parameter.name)
                    .and_then(|values| values.get(index))
                {
                    instance.inject(
                        parameter.name.clone(),
                        value.clone(),
                    );
                }
            }
            let _ = name;
            output.push_str(&instance.render()?);
        }
        Ok(())
    }

    fn push_text_slice(
        slices: &mut Vec<PromptSlice>,
        text: &str,
        offset: usize,
    ) {
        if let Some(relative_start) = text.find(MARKER_OPENING) {
            let start = offset + relative_start;
            panic!(
                "malformed prompt marker at byte {start}: expected \
                 `{{{{#name}}}}`, `{{{{^#name}}}}`, \
                 `{{{{@module}}}}`, or `{{{{^@module}}}}`"
            );
        }
        if !text.is_empty() {
            slices.push(PromptSlice::Text(text.to_owned()));
        }
    }

    fn marker() -> &'static regex::Regex {
        MARKER.get_or_init(|| {
            regex::Regex::new(MARKER_PATTERN)
                .unwrap_or_else(|error| {
                    panic!("invalid prompt marker regex: {error}")
                })
        })
    }

    fn module_file_name(name: &str) -> String {
        let mut output = String::with_capacity(name.len());
        let mut uppercase = true;
        for character in name.chars() {
            if character == '_' {
                uppercase = true;
            } else if uppercase {
                output.extend(character.to_uppercase());
                uppercase = false;
            } else {
                output.push(character);
            }
        }
        output
    }

    fn assert_identifier(kind: &str, name: &str) {
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| {
                    byte.is_ascii_alphanumeric() || byte == b'_'
                })
        {
            panic!(
                "invalid {kind} name `{name}`: expected only ASCII \
                 letters, digits, or underscore"
            );
        }
    }

    fn read(path: &Path) -> String {
        fs::read_to_string(path).unwrap_or_else(|error| {
            panic!("failed to read prompt {}: {error}", path.display())
        })
    }
}
