use std::collections::BTreeMap;

use marix_protocol::{ExecutionSignature, ExecutionStatus, StepSignature};

#[derive(Debug, Clone)]
pub struct Execution {
    pub signature: ExecutionSignature,
    pub step: StepSignature,
    pub status: ExecutionStatus,
    pub output: BTreeMap<usize, String>,
    pub final_signal: Option<usize>,
}

impl Execution {
    pub fn new(signature: ExecutionSignature, step: StepSignature) -> Self {
        Self {
            signature,
            step,
            status: ExecutionStatus::Started,
            output: BTreeMap::new(),
            final_signal: None,
        }
    }

    pub fn push(&mut self, seq: usize, content: String) -> bool {
        self.output.insert(seq, content);
        self.is_complete()
    }

    pub fn finalize(&mut self, count: usize) -> bool {
        self.final_signal = Some(count);
        self.status = ExecutionStatus::Succeed(count);
        self.is_complete()
    }

    pub fn content(&self) -> String {
        self.output.values().cloned().collect()
    }
}

// -- Private -- //

impl Execution {
    fn is_complete(&self) -> bool {
        self.final_signal
            .is_some_and(|count| self.output.len() == count)
    }
}
