use std::env;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use super::sys::System;
use crate::external::*;

pub const CONFIG_FILE: &str = "config.toml";
const CONFIG_ENV_VAR: &str = "MARIX_CONFIG";
static CONFIG_CACHE: RwLock<Option<Result<Config, String>>> = RwLock::new(None);

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub name: String,
    #[serde(skip, default = "System::new")]
    pub system: System,
    pub runtime: RuntimeConfig,
    #[serde(default = "default_core_config")]
    pub core: CoreConfig,
    pub client: ClientConfig,
    pub server: ServerConfig,
    pub model: ModelConfig,
    pub tool: ToolConfig,
}

impl Config {
    pub fn load() -> Result<Self, String> {
        if let Some(cached) = CONFIG_CACHE
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .as_ref()
        {
            return cached.clone();
        }

        let computed = config_path()
            .and_then(|path| load_config(&path))
            .map_err(|error| error.to_string());
        CONFIG_CACHE
            .write()
            .unwrap_or_else(|error| error.into_inner())
            .get_or_insert(computed)
            .clone()
    }

    /// Builds a test config: loads the base config, applies each TOML
    /// fragment on top (later fragments win on conflict), installs the
    /// result as the active config so subsequent `Config::load()` calls
    /// return it, and returns it. Intended for tests.
    pub fn mock(overrides: &[&str]) -> Result<Self, String> {
        Self::load()?;
        let path = config_path().map_err(|error| error.to_string())?;
        let repo_root = repository_root_for_config(&path);
        let content = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
        let mut table: toml::Table = toml::from_str(&content).map_err(|error| error.to_string())?;
        for fragment in overrides {
            let overlay: toml::Table =
                toml::from_str(fragment).map_err(|error| error.to_string())?;
            merge_tables(&mut table, overlay);
        }
        let merged = toml::to_string(&table).map_err(|error| error.to_string())?;
        let config = build_config(&merged, &repo_root).map_err(|error| error.to_string())?;
        *CONFIG_CACHE
            .write()
            .unwrap_or_else(|error| error.into_inner()) = Some(Ok(config.clone()));
        Ok(config)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeConfig {
    pub environment: RuntimeEnvironment,
    pub mode: RuntimeMode,
    #[serde(default = "default_marix_path")]
    pub marix_path: String,
    #[serde(default)]
    pub marix_path_client: Option<String>,
    #[serde(default)]
    pub marix_path_server: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeEnvironment {
    Development,
    Test,
    Production,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeMode {
    Local,
    Ipc,
    Network,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoreConfig {
    pub bind_address: String,
    pub worker_threads: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClientConfig {
    pub interactive: bool,
    pub request_timeout_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerConfig {
    pub enabled: bool,
    pub ip: String,
    pub auth_token: String,
    pub client_port: u16,
    pub host_port: u16,
    pub telemetry_port: u16,
    pub telemetry_http_port: u16,
    pub max_turns: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelConfig {
    pub selected: String,
    pub deepseek: DeepseekConfig,
    pub glm: GlmConfig,
}

#[derive(Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeepseekConfig {
    pub endpoint: String,
    pub model: String,
    pub api_key: String,
}

#[derive(Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GlmConfig {
    pub endpoint: String,
    pub model: String,
    pub api_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolConfig {
    pub directory: String,
}

// -- Private -- //

fn config_path() -> Result<PathBuf, ConfigError> {
    if let Some(path) = env::var_os(CONFIG_ENV_VAR).filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(path));
    }

    let executable = env::current_exe().map_err(|error| {
        std::io::Error::new(
            error.kind(),
            format!("failed to resolve current executable path: {error}"),
        )
    })?;
    let parent = executable
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!(
                    "current executable path '{}' has no parent directory",
                    executable.display()
                ),
            )
        })?;
    Ok(parent.join(CONFIG_FILE))
}

fn load_config(config_path: &Path) -> Result<Config, ConfigError> {
    let content = std::fs::read_to_string(config_path)?;
    let repo_root = repository_root_for_config(config_path);
    build_config(&content, &repo_root)
}

fn build_config(content: &str, repo_root: &Path) -> Result<Config, ConfigError> {
    let mut config: Config = toml::from_str(content)?;
    config.runtime = resolve_runtime_paths(repo_root, config.runtime);
    config.tool.directory =
        path_to_config_string(resolve_config_path(repo_root, &config.tool.directory));
    Ok(config)
}

fn merge_tables(base: &mut toml::Table, overlay: toml::Table) {
    for (key, overlay_value) in overlay {
        match (base.get_mut(&key), overlay_value) {
            (Some(toml::Value::Table(base_table)), toml::Value::Table(overlay_table)) => {
                merge_tables(base_table, overlay_table);
            }
            (_, overlay_value) => {
                base.insert(key, overlay_value);
            }
        }
    }
}

fn default_core_config() -> CoreConfig {
    CoreConfig {
        bind_address: "127.0.0.1:0".to_owned(),
        worker_threads: 1,
    }
}

fn default_marix_path() -> String {
    ".".to_owned()
}

fn resolve_runtime_paths(repo_root: &Path, mut runtime: RuntimeConfig) -> RuntimeConfig {
    runtime.marix_path = resolve_required_runtime_path(repo_root, &runtime.marix_path);
    runtime.marix_path_client = resolve_optional_runtime_path(repo_root, runtime.marix_path_client);
    runtime.marix_path_server = resolve_optional_runtime_path(repo_root, runtime.marix_path_server);
    runtime
}

fn resolve_required_runtime_path(repo_root: &Path, configured_path: &str) -> String {
    let trimmed_path = configured_path.trim();
    let path = if trimmed_path.is_empty() {
        repo_root.to_path_buf()
    } else {
        resolve_config_path(repo_root, trimmed_path)
    };
    path_to_config_string(path)
}

fn resolve_optional_runtime_path(
    repo_root: &Path,
    configured_path: Option<String>,
) -> Option<String> {
    configured_path.and_then(|path| {
        let trimmed_path = path.trim();
        if trimmed_path.is_empty() {
            None
        } else {
            Some(path_to_config_string(resolve_config_path(
                repo_root,
                trimmed_path,
            )))
        }
    })
}

fn path_to_config_string(path: PathBuf) -> String {
    path.to_string_lossy().into_owned()
}

fn repository_root_for_config(config_path: &Path) -> PathBuf {
    let config_dir = path_parent_or_current(config_path);
    if config_dir.file_name().and_then(|name| name.to_str()) == Some("src") {
        path_parent_or_current(config_dir).to_path_buf()
    } else {
        config_dir.to_path_buf()
    }
}

fn path_parent_or_current(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn resolve_config_path(root: &Path, configured_path: &str) -> PathBuf {
    let path = Path::new(configured_path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

#[derive(Debug)]
enum ConfigError {
    Io(std::io::Error),
    Toml(toml::de::Error),
}

impl From<std::io::Error> for ConfigError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(error: toml::de::Error) -> Self {
        Self::Toml(error)
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "I/O error: {error}"),
            Self::Toml(error) => write!(formatter, "TOML error: {error}"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl fmt::Debug for DeepseekConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DeepseekConfig")
            .field("endpoint", &self.endpoint)
            .field("model", &self.model)
            .field("api_key_configured", &(!self.api_key.is_empty()))
            .finish()
    }
}

impl fmt::Debug for GlmConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GlmConfig")
            .field("endpoint", &self.endpoint)
            .field("model", &self.model)
            .field("api_key_configured", &(!self.api_key.is_empty()))
            .finish()
    }
}
