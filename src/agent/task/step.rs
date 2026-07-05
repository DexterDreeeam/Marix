use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use marix_protocol::{StepSignature, StepStatus};

#[derive(Debug, Clone)]
pub struct Step {
    pub signature: StepSignature,
    pub status: StepStatus,
    pub update_count: Arc<AtomicUsize>,
}

impl Step {
    pub fn new(signature: StepSignature) -> Self {
        Self {
            signature,
            status: StepStatus::Prepare,
            update_count: Arc::new(AtomicUsize::new(0)),
        }
    }
}
