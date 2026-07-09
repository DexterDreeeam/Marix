use marix_common::{Receiver, Sender, SharedNetSender, WorkQueue, build_channel};
use marix_protocol::{ExecutionSignature, ExecutorEvent, SessionMessage};

use crate::execution::Execution;
use crate::executor::ToolRegistry;

pub(super) struct ExecutorState {
    pub(super) registry: ToolRegistry,
    pub(super) executions: WorkQueue<ExecutionSignature, Execution>,
    pub(super) executor_tx: Sender<ExecutorEvent>,
    pub(super) executor_rx: Receiver<ExecutorEvent>,
    pub(super) server_tx: SharedNetSender<SessionMessage>,
}

impl ExecutorState {
    pub(super) fn new(server_tx: SharedNetSender<SessionMessage>) -> Self {
        let (executor_tx, executor_rx) = build_channel();
        Self {
            registry: ToolRegistry::new(),
            executions: WorkQueue::new(),
            executor_tx,
            executor_rx,
            server_tx,
        }
    }
}
