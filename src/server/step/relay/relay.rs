use std::collections::BTreeMap;

use marix_protocol::{RelaySignature, RelayStatus, StepSignature};

#[derive(Debug, Clone)]
pub struct Relay {
    pub signature: RelaySignature,
    pub step: StepSignature,
    pub status: RelayStatus,
    pub output: BTreeMap<usize, String>,
    pub final_signal: Option<usize>,
}

impl Relay {
    pub fn new(signature: RelaySignature, step: StepSignature) -> Self {
        panic!("not implemented")
    }

    pub fn push(&mut self, seq: usize, content: String) -> bool {
        panic!("not implemented")
    }

    pub fn finalize(&mut self, count: usize) -> bool {
        panic!("not implemented")
    }

    pub fn content(&self) -> String {
        panic!("not implemented")
    }
}

// -- Private -- //

impl Relay {
    fn is_complete(&self) -> bool {
        panic!("not implemented")
    }
}
