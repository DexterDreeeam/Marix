use std::collections::BTreeMap;

use marix_protocol::{ExecutionSignature, ExecutionStatus};

#[derive(Debug, Clone)]
pub struct Execution {
    pub signature: ExecutionSignature,
    pub status: ExecutionStatus,
    pub output: BTreeMap<usize, String>,
}

impl Execution {
    pub fn new(signature: ExecutionSignature) -> Self {
        Self {
            signature,
            status: ExecutionStatus::Started,
            output: BTreeMap::new(),
        }
    }
}
