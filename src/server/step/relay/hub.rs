use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use marix_protocol::{RelayEvent, RelaySignature, RelayStatus};

use crate::step::Step;
use crate::step::relay::Relay;
use crate::task::TaskState;

pub struct RelayHub {
    relay_map: Mutex<HashMap<RelaySignature, Relay>>,
}

impl RelayHub {
    pub fn new() -> Self {
        panic!("not implemented")
    }

    pub(crate) fn run_relay_step(&self, state: &Arc<TaskState>, step: Step) {
        panic!("not implemented")
    }

    pub(crate) fn route_event(
        &self,
        state: &Arc<TaskState>,
        signature: RelaySignature,
        event: RelayEvent,
    ) {
        panic!("not implemented")
    }

    pub fn status(&self, signature: &RelaySignature) -> RelayStatus {
        panic!("not implemented")
    }
}

// -- Private -- //

impl RelayHub {
    fn on_complete(&self, state: &Arc<TaskState>, signature: &RelaySignature) {
        panic!("not implemented")
    }
}
