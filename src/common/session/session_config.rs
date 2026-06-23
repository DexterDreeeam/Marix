use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const DEFAULT_CORE_PORT: u16 = 0;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionConfig {
    pub core_ip: String,
    pub core_port: u16,
}

impl SessionConfig {
    pub fn new(config: &Value) -> Self {
        let session = config.get("cli").and_then(|cli| cli.get("session"));
        Self {
            core_ip: session
                .and_then(|session| session.get("core_ip"))
                .and_then(Value::as_str)
                .unwrap_or("127.0.0.1")
                .to_owned(),
            core_port: session
                .and_then(|session| session.get("core_port"))
                .and_then(value_as_u16)
                .unwrap_or(DEFAULT_CORE_PORT),
        }
    }

    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.core_ip, self.core_port)
    }
}

fn value_as_u16(value: &Value) -> Option<u16> {
    match value {
        Value::Number(number) => number.as_u64().and_then(u64_to_u16),
        Value::String(text) => text.parse::<u16>().ok(),
        _ => None,
    }
}

fn u64_to_u16(port: u64) -> Option<u16> {
    u16::try_from(port).ok()
}
