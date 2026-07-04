use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use marix_protocol::{StepSignature, StepStatus};

#[derive(Debug, Clone)]
pub struct Step {
    pub signature: StepSignature,
    pub status: StepStatus,
    pub update_count: Arc<AtomicUsize>,
}
