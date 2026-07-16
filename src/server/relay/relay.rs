use std::sync::Arc;

use marix_common::Logger;
use marix_protocol::{RelayEvent, RelayRequest, RelayResult, RelayStatus};

use super::{RelayRuntime, RelayState};
use crate::task::TaskAccess;

#[derive(Clone)]
pub struct Relay {
    pub state: Arc<RelayState>,
}

impl Relay {
    pub fn status(&self) -> RelayStatus {
        self.state
            .status
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    pub fn result(&self) -> Option<RelayResult> {
        match self.status() {
            RelayStatus::Complete(result) => Some(result),
            RelayStatus::Created | RelayStatus::Running => None,
        }
    }

    pub fn start(&self) {
        let runtime = RelayRuntime::new(Arc::clone(&self.state));
        drop(self.state.access.rt.spawn(async move {
            runtime.run().await;
        }));
    }

    pub fn dispatch(&self, event: RelayEvent) {
        if self.state.relay_tx.send(event).is_err() {
            Logger::warning(format!(
                "relay {} event dispatch failed: worker stopped",
                &self.state.signature,
            ));
        }
    }
}

// -- Private -- //

impl Relay {
    pub(crate) fn new(
        access: Arc<TaskAccess>,
        request: RelayRequest,
    ) -> Result<Self, String> {
        let state = Arc::new(RelayState::new(access, request)?);
        Ok(Self { state })
    }
}
