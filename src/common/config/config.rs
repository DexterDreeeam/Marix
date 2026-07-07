use std::env;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use super::sys::System;
use crate::external::*;

pub const CONFIG_FILE: &str = "src/config.toml";
const CONFIG_ENV_VAR: &str = "MARIX_CONFIG";
const DEPLOYED_CONFIG_FILE: &str = "config.toml";
static CONFIG_CACHE: OnceLock<Result<Config, String>> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub name: String,
    pub system: System,
    pub runtime: RuntimeConfig,
    pub core: CoreConfig,
    pub client: ClientConfig,
    pub server: ServerConfig,
    pub model: ModelConfig,
    pub credential: CredentialConfig,
    pub tool: ToolConfig,
}

impl Config {
    pub fn load() -> Result<Self, String> {
        CONFIG_CACHE
            .get_or_init(|| load_config(&config_path()).map_err(|error| error.to_string()))
            .clone()
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
    pub client_port: u16,
    pub host_port: u16,
    pub telemetry_port: u16,
    pub max_turns: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelConfig {
    pub backend: ModelBackend,
    pub deepseek: DeepseekConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelBackend {
    Deepseek,
}

#[derive(Clone, PartialEq, Eq)]
pub struct DeepseekConfig {
    pub endpoint: String,
    pub model: String,
    pub api_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CredentialConfig {
    pub directory: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolConfig {
    pub directory: String,
}

// -- Private -- //

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    name: String,
    runtime: RuntimeConfig,
    core: Option<CoreConfig>,
    client: ClientConfig,
    server: RawServerConfig,
    model: RawModelConfig,
    credential: CredentialConfig,
    tool: ToolConfig,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawServerConfig {
    enabled: bool,
    ip: CredentialRef,
    client_port: u16,
    host_port: u16,
    telemetry_port: u16,
    max_turns: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawModelConfig {
    backend: ModelBackend,
    deepseek: RawDeepseekConfig,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDeepseekConfig {
    endpoint: String,
    model: String,
    api_key: CredentialRef,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CredentialRef {
    name: String,
}

fn config_path() -> PathBuf {
    if let Some(path) = env::var_os(CONFIG_ENV_VAR).filter(|value| !value.is_empty()) {
        return PathBuf::from(path);
    }

    let source_config = PathBuf::from(CONFIG_FILE);
    if source_config.is_file() {
        return source_config;
    }

    PathBuf::from(DEPLOYED_CONFIG_FILE)
}

fn load_config(config_path: &Path) -> Result<Config, ConfigError> {
    let system = System::new();
    let repo_root = repository_root_for_config(config_path);
    let content = std::fs::read_to_string(config_path)?;
    let raw_config: RawConfig = toml::from_str(&content)?;
    let credential_root = resolve_config_path(&repo_root, &raw_config.credential.directory);
    let server_ip = read_credential(&credential_root, &raw_config.server.ip)?;
    let deepseek_api_key = read_credential(&credential_root, &raw_config.model.deepseek.api_key)?;

    let runtime = resolve_runtime_paths(&repo_root, raw_config.runtime);

    Ok(Config {
        name: raw_config.name,
        system,
        runtime,
        core: raw_config.core.unwrap_or_else(default_core_config),
        client: raw_config.client,
        server: ServerConfig {
            enabled: raw_config.server.enabled,
            ip: server_ip,
            client_port: raw_config.server.client_port,
            host_port: raw_config.server.host_port,
            telemetry_port: raw_config.server.telemetry_port,
            max_turns: raw_config.server.max_turns,
        },
        model: ModelConfig {
            backend: raw_config.model.backend,
            deepseek: DeepseekConfig {
                endpoint: raw_config.model.deepseek.endpoint,
                model: raw_config.model.deepseek.model,
                api_key: deepseek_api_key,
            },
        },
        credential: raw_config.credential,
        tool: ToolConfig {
            directory: path_to_config_string(resolve_config_path(
                &repo_root,
                &raw_config.tool.directory,
            )),
        },
    })
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

fn read_credential(
    credential_root: &Path,
    credential_ref: &CredentialRef,
) -> Result<String, ConfigError> {
    let path = credential_root.join(format!("{}.txt", credential_ref.name));
    let value = std::fs::read_to_string(&path)?.trim().to_owned();
    if value.is_empty() {
        return Err(ConfigError::EmptyCredential(credential_ref.name.clone()));
    }
    Ok(value)
}

#[derive(Debug)]
enum ConfigError {
    Io(std::io::Error),
    Toml(toml::de::Error),
    EmptyCredential(String),
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
            Self::EmptyCredential(name) => write!(formatter, "credential is empty: {name}"),
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
