use std::collections::HashMap;
use std::sync::Arc;

use marix_protocol::{ExecutionEvent, ExecutionSignature, ExecutionStatus};

use crate::execution::Execution;
use crate::task::TaskState;

pub struct ExecutionHub {
    task_state: Arc<TaskState>,
    execution_map: HashMap<ExecutionSignature, Execution>,
}

impl ExecutionHub {
    pub fn new(task_state: Arc<TaskState>) -> Self {
        Self {
            task_state,
            execution_map: HashMap::new(),
        }
    }

    pub fn route_event(&mut self, _signature: ExecutionSignature, _event: ExecutionEvent) {
        panic!("not implemented")
    }

    pub fn status(&self, signature: &ExecutionSignature) -> ExecutionStatus {
        // An untracked signature has not started yet, so it reports Started.
        self.execution_map
            .get(signature)
            .map(|execution| execution.status.clone())
            .unwrap_or(ExecutionStatus::Started)
    }
}
