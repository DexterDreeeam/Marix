use std::time::{SystemTime, UNIX_EPOCH};

use crate::SessionEvent;
use crate::external::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionMessage {
    pub timestamp: String,
    pub source_name: String,
    pub event: SessionEvent,
}

impl SessionMessage {
    pub fn new(source_name: String, event: SessionEvent) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_millis().to_string())
            .unwrap_or_default();
        Self {
            timestamp,
            source_name,
            event,
        }
    }
}
