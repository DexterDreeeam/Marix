use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use marix_common::Logger;
use marix_protocol::{RelayRequest, RelaySignature, RelayStatus};

use crate::relay::Relay;
use crate::task::TaskState;

pub struct RelayHub {
    relay_map: Mutex<HashMap<RelaySignature, Relay>>,
}

impl RelayHub {
    pub fn new() -> Self {
        Self {
            relay_map: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn create(&self, state: &Arc<TaskState>, request: RelayRequest) -> Option<Relay> {
        let signature = request.signature.clone();
        let relay = Relay::new(Arc::clone(state), request);
        let mut relays = self
            .relay_map
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if relays.contains_key(&signature) {
            Logger::warning(format!(
                "relay {} create ignored: relay already exists",
                signature.relay_id.0
            ));
            return None;
        }
        relays.insert(signature, relay.clone());
        Some(relay)
    }

    pub fn status(&self, signature: &RelaySignature) -> RelayStatus {
        self.relay_map
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(signature)
            .map(Relay::status)
            .unwrap_or(RelayStatus::Created)
    }

    pub(crate) fn with<R>(
        &self,
        signature: &RelaySignature,
        function: impl FnOnce(&Relay) -> R,
    ) -> Option<R> {
        self.relay_map
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(signature)
            .map(function)
    }
}
