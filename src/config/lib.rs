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
        let mut loaded = Self::load_src_configs(src_root)?;
        if deployment_path.as_ref().exists() {
            loaded.set("deployment", read_json_file(deployment_path)?);
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
        let mut data = Value::Object(Map::new());
        for path in find_config_files(src_root.as_ref())? {
            merge_json(&mut data, read_json_file(path)?);
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

fn read_json_file(path: impl AsRef<Path>) -> Result<Value, ConfigError> {
    let content = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&content)?)
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

#[cfg(test)]
mod tests {
    use super::{config, merge_json, Config};
    use serde_json::json;

    #[test]
    fn indexes_config_by_string_key() {
        let mut loaded = Config::empty();
        loaded.set("cli", json!({ "interface": "cli" }));
        assert_eq!(loaded["cli"]["interface"], "cli");
    }

    #[test]
    fn recursively_merges_config_values() {
        let mut target = json!({ "core": { "mode": "upxcy_m" } });
        merge_json(
            &mut target,
            json!({ "core": { "model": "deepseek-default" } }),
        );
        assert_eq!(target["core"]["mode"], "upxcy_m");
        assert_eq!(target["core"]["model"], "deepseek-default");
    }

    #[test]
    fn global_config_loads_on_first_index() {
        assert_eq!(config["cli"]["interface"], "cli");
    }
}
