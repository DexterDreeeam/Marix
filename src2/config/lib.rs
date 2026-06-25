use std::collections::BTreeMap;
use std::ops::Index;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde_json::{Map, Value};

static GLOBAL_CONFIG: OnceLock<Config> = OnceLock::new();

#[allow(non_upper_case_globals)]
pub static config: IConfig = IConfig;

#[derive(Debug, Clone, Copy)]
pub struct IConfig;

impl IConfig {
    pub fn current(self) -> &'static Config {
        GLOBAL_CONFIG.get_or_init(|| {
            Config::load_from_src_root(default_src_root())
                .expect("failed to load Marix config from src/**/config.json and deployment.json")
        })
    }

    pub fn as_value(self) -> &'static Value {
        self.current().as_value()
    }
}

impl Index<&str> for IConfig {
    type Output = Value;

    fn index(&self, index: &str) -> &Self::Output {
        &self.current()[index]
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    data: Value,
}

impl Config {
    pub fn empty() -> Self {
        Self {
            data: Value::Object(Map::new()),
        }
    }

    pub fn load(
        src_root: impl AsRef<Path>,
        deployment_path: impl AsRef<Path>,
    ) -> Result<Self, ConfigError> {
        let src_root = src_root.as_ref();
        let aliases = load_aliases(&alias_root_for_src(src_root))?;
        let mut loaded = Self::load_src_configs_with_aliases(src_root, &aliases)?;
        if deployment_path.as_ref().exists() {
            loaded.set("deployment", read_json_file(deployment_path, &aliases)?);
        }
        Ok(loaded)
    }

    pub fn load_from_src_root(src_root: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let src_root = src_root.as_ref();
        let deployment_path = src_root
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("deployment.json");
        Self::load(src_root, deployment_path)
    }

    pub fn load_src_configs(src_root: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let src_root = src_root.as_ref();
        let aliases = load_aliases(&alias_root_for_src(src_root))?;
        Self::load_src_configs_with_aliases(src_root, &aliases)
    }

    fn load_src_configs_with_aliases(
        src_root: &Path,
        aliases: &BTreeMap<String, String>,
    ) -> Result<Self, ConfigError> {
        let mut data = Value::Object(Map::new());
        for path in find_config_files(src_root)? {
            merge_json(&mut data, read_json_file(path, aliases)?);
        }
        Ok(Self { data })
    }

    pub fn set(&mut self, key: impl Into<String>, value: Value) {
        if !self.data.is_object() {
            self.data = Value::Object(Map::new());
        }
        if let Some(object) = self.data.as_object_mut() {
            object.insert(key.into(), value);
        }
    }

    pub fn as_value(&self) -> &Value {
        &self.data
    }

    pub fn into_value(self) -> Value {
        self.data
    }
}

impl Index<&str> for Config {
    type Output = Value;

    fn index(&self, index: &str) -> &Self::Output {
        &self.data[index]
    }
}

fn default_src_root() -> PathBuf {
    std::env::var_os("MARIX_SRC_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")))
        })
}

fn read_json_file(
    path: impl AsRef<Path>,
    aliases: &BTreeMap<String, String>,
) -> Result<Value, ConfigError> {
    let content = std::fs::read_to_string(path)?;
    let mut value = serde_json::from_str(&content)?;
    resolve_value_aliases(&mut value, aliases);
    Ok(value)
}

fn alias_root_for_src(src_root: &Path) -> PathBuf {
    src_root
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(".alias")
}

fn load_aliases(alias_root: &Path) -> Result<BTreeMap<String, String>, ConfigError> {
    let mut aliases = BTreeMap::new();
    if !alias_root.is_dir() {
        return Ok(aliases);
    }
    for entry in std::fs::read_dir(alias_root)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file()
            || path.extension().and_then(|extension| extension.to_str()) != Some("txt")
        {
            continue;
        }
        let Some(key) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let value = std::fs::read_to_string(&path)?.trim().to_owned();
        if !value.is_empty() {
            aliases.insert(key.to_owned(), value);
        }
    }
    Ok(aliases)
}

fn resolve_aliases(content: &str, aliases: &BTreeMap<String, String>) -> String {
    let mut resolved = String::with_capacity(content.len());
    let mut index = 0;
    while let Some(start_offset) = content[index..].find("{{") {
        let start = index + start_offset;
        resolved.push_str(&content[index..start]);
        let value_start = start + 2;
        let Some(end_offset) = content[value_start..].find("}}") else {
            resolved.push_str(&content[start..]);
            return resolved;
        };
        let end = value_start + end_offset;
        let key = content[value_start..end].trim();
        if let Some(value) = aliases.get(key) {
            resolved.push_str(value);
        } else {
            resolved.push_str(&content[start..end + 2]);
        }
        index = end + 2;
    }
    resolved.push_str(&content[index..]);
    resolved
}

fn resolve_value_aliases(value: &mut Value, aliases: &BTreeMap<String, String>) {
    match value {
        Value::String(text) => {
            *text = resolve_aliases(text, aliases);
        }
        Value::Array(items) => {
            for item in items {
                resolve_value_aliases(item, aliases);
            }
        }
        Value::Object(entries) => {
            for item in entries.values_mut() {
                resolve_value_aliases(item, aliases);
            }
        }
        _ => {}
    }
}

fn find_config_files(root: &Path) -> Result<Vec<PathBuf>, ConfigError> {
    let mut files = Vec::new();
    visit_config_files(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn visit_config_files(path: &Path, files: &mut Vec<PathBuf>) -> Result<(), ConfigError> {
    if is_dot_path(path) {
        return Ok(());
    }
    if path.is_file() {
        if path.file_name().and_then(|name| name.to_str()) == Some("config.json") {
            files.push(path.to_path_buf());
        }
        return Ok(());
    }
    if !path.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        visit_config_files(&entry.path(), files)?;
    }
    Ok(())
}

fn is_dot_path(path: &Path) -> bool {
    path.components().any(|component| {
        component
            .as_os_str()
            .to_str()
            .is_some_and(|part| part.starts_with('.'))
    })
}

fn merge_json(target: &mut Value, source: Value) {
    match (target, source) {
        (Value::Object(target), Value::Object(source)) => {
            for (key, value) in source {
                merge_json(target.entry(key).or_insert(Value::Null), value);
            }
        }
        (target, source) => {
            *target = source;
        }
    }
}

#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl From<std::io::Error> for ConfigError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for ConfigError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "config I/O error: {error}"),
            Self::Json(error) => write!(formatter, "config JSON error: {error}"),
        }
    }
}

impl std::error::Error for ConfigError {}
