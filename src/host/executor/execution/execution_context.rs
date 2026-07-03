use marix_common::{ExecutionSignature, SessionEvent, SharedNetSender};

pub struct ExecutionContext {
    pub signature: ExecutionSignature,
    pub agent_tx: SharedNetSender<SessionEvent>,
}

impl ExecutionContext {
    pub fn new(signature: ExecutionSignature, agent_tx: SharedNetSender<SessionEvent>) -> Self {
        panic!("not implemented")
    }
}
