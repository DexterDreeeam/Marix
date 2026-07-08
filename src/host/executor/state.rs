use marix_common::{Sender, WorkQueue};
use marix_protocol::{ExecutionSignature, ExecutorEvent};

use crate::executor::{Execution, ToolRegistry};

pub(super) struct ExecutorState {
    pub(super) registry: ToolRegistry,
    pub(super) executions: WorkQueue<ExecutionSignature, Execution>,
    pub(super) executor_tx: Sender<ExecutorEvent>,
}

impl ExecutorState {
    pub(super) fn new(executor_tx: Sender<ExecutorEvent>) -> Self {
        Self {
            registry: ToolRegistry::new(),
            executions: WorkQueue::new(),
            executor_tx,
        }
    }
}
