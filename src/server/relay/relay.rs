use std::fmt;
use std::sync::Arc;

use marix_common::Logger;
use marix_protocol::{Actor, RelayEvent, RelayRequest, RelaySignature, RuntimeAsync};

use super::runtime::RelayRuntime;
use super::state::RelayState;
use crate::task::TaskAccess;

pub struct Relay {
    state: Arc<RelayState>,
}

impl Clone for Relay {
    fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
        }
    }
}

impl Relay {
    pub fn new(access: TaskAccess, request: RelayRequest) -> Self {
        let signature = request.signature.clone();
        let state = Arc::new(RelayState::new(access, signature, request));
        Self { state }
    }

    pub(crate) fn signature(&self) -> &RelaySignature {
        &self.state.signature
    }
}

impl Actor<Relay, RelayEvent> for Relay {
    fn start(&mut self) {
        let runtime = RelayRuntime::new(Arc::clone(&self.state));
        drop(self.state.access.rt.spawn(async move {
            runtime.run().await;
        }));
    }

    fn dispatch(&self, event: RelayEvent) {
        if self.state.relay_tx.send(event).is_err() {
            Logger::warning(format!(
                "relay {} event dispatch failed: worker stopped",
                &self.state.signature,
            ));
        }
    }
}

impl fmt::Debug for Relay {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Relay")
            .field("signature", &self.state.signature)
            .field("step", &self.state.signature.step)
            .finish_non_exhaustive()
    }
}
