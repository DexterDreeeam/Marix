use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const DEFAULT_CORE_PORT: u16 = 22345;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionConfig {
    pub remote_core: bool,
    pub core_ip: String,
    pub core_port: u16,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            remote_core: false,
            core_ip: "127.0.0.1".to_owned(),
            core_port: DEFAULT_CORE_PORT,
        }
    }
}

impl SessionConfig {
    pub fn integrated_core() -> Self {
        Self::default()
    }

    pub fn remote_core(core_ip: impl Into<String>, core_port: u16) -> Self {
        Self {
            remote_core: true,
            core_ip: core_ip.into(),
            core_port,
        }
    }

    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.core_ip, self.core_port)
    }

    pub fn from_cli_config_value(value: &Value) -> Self {
        Self::from_candidates([
            value.get("cli").and_then(|cli| cli.get("session")),
            value.get("session"),
            Some(value),
        ])
    }

    pub fn from_core_config_value(value: &Value) -> Self {
        Self::from_candidates([
            value.get("core").and_then(|core| core.get("session")),
            value.get("session"),
            Some(value),
        ])
    }

    pub fn from_config_value(value: &Value) -> Self {
        Self::from_candidates([value.get("session"), Some(value), None])
    }

    fn from_candidates<const N: usize>(candidates: [Option<&Value>; N]) -> Self {
        let mut config = Self::default();
        for candidate in candidates.into_iter().rev().flatten() {
            if let Some(remote_core) = candidate.get("remote_core").and_then(Value::as_bool) {
                config.remote_core = remote_core;
            }
            if let Some(core_ip) = candidate.get("core_ip").and_then(Value::as_str) {
                config.core_ip = core_ip.to_owned();
            }
            if let Some(core_port) = candidate
                .get("core_port")
                .and_then(Value::as_u64)
                .and_then(|port| u16::try_from(port).ok())
            {
                config.core_port = core_port;
            }
        }
        config
    }
}
