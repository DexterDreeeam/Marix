use std::sync::mpsc;

use crate::agent::frontdoor::AgentTask;

pub(crate) struct SessionContext {
    accepted_task_tx: mpsc::Sender<AgentTask>,
    accepted_task_rx: mpsc::Receiver<AgentTask>,
}

impl SessionContext {
    pub(crate) fn new() -> Self {
        let (accepted_task_tx, accepted_task_rx) = mpsc::channel();
        Self {
            accepted_task_tx,
            accepted_task_rx,
        }
    }

    pub(crate) fn task_sender(&self) -> mpsc::Sender<AgentTask> {
        self.accepted_task_tx.clone()
    }

    pub(crate) fn next_task(&self) -> Option<AgentTask> {
        self.accepted_task_rx.try_recv().ok()
    }

    pub(crate) fn drain_tasks(&self) {
        while self.accepted_task_rx.try_recv().is_ok() {}
    }
}
