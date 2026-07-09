use std::collections::HashMap;
use std::sync::Mutex;

use marix_common::Logger;
use marix_protocol::{Actor, RelayRequest, RelaySignature};

use crate::relay::Relay;
use crate::task::TaskAccess;

pub struct RelayHub {
    relay_map: Mutex<HashMap<RelaySignature, Relay>>,
}

impl RelayHub {
    pub fn new() -> Self {
        Self {
            relay_map: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn create(&self, access: TaskAccess, request: RelayRequest) -> bool {
        let signature = request.signature.clone();
        let mut relays = self
            .relay_map
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if relays.contains_key(&signature) {
            Logger::warning(format!(
                "relay {} create ignored: relay already exists",
                &signature,
            ));
            return false;
        }
        let mut relay = Relay::new(access, request);
        relay.start();
        relays.insert(signature, relay);
        true
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
