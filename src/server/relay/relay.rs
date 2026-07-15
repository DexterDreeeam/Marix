use std::sync::Arc;

use marix_common::Logger;
use marix_protocol::{RelayEvent, RelayRequest, RelayStatus};

use super::{RelayRuntime, RelayState};
use crate::task::TaskAccess;

#[derive(Clone)]
pub struct Relay {
    pub access: Arc<TaskAccess>,
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

    pub fn result(&self) -> Option<String> {
        if !matches!(self.status(), RelayStatus::Succeed { .. }) {
            return None;
        }
        let count = self
            .state
            .final_signal
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_ref()
            .copied()?;
        let output = self
            .state
            .output
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if (0..count).any(|seq| !output.contains_key(&seq)) {
            return None;
        }
        Some(
            (0..count)
                .filter_map(|seq| output.get(&seq))
                .cloned()
                .collect(),
        )
    }

    pub fn start(&self) {
        let runtime = RelayRuntime::new(Arc::clone(&self.access), Arc::clone(&self.state));
        drop(self.access.rt.spawn(async move {
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
        let state = Arc::new(RelayState::new(Arc::clone(&access), request)?);
        Ok(Self { access, state })
    }
}
