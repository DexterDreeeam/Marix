use std::sync::Arc;
use std::thread::JoinHandle;

use marix_common::{ExecutionSignature, Receiver, Sender, SessionEvent, SharedNetSender};

use super::ExecutionContext;

pub struct ExecutionRuntime {
    context: Arc<ExecutionContext>,
    execution_tx: Sender<SessionEvent>,
    execution_rx: Option<Receiver<SessionEvent>>,
    worker: Option<JoinHandle<()>>,
}

impl ExecutionRuntime {
    pub fn new(signature: ExecutionSignature, agent_tx: SharedNetSender<SessionEvent>) -> Self {
        panic!("not implemented")
    }

    pub fn sender(&self) -> Sender<SessionEvent> {
        panic!("not implemented")
    }

    pub fn run(&mut self) {
        panic!("not implemented")
    }
}
