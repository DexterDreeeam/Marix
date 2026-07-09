use crate::executor::Tool;
use marix_common::{Receiver, Sender, SharedNetSender, build_channel};
use marix_protocol::{ExecutionEvent, ExecutionRequest, SessionMessage};

pub struct ExecutionState {
    pub(super) tool: Tool,
    pub(super) request: ExecutionRequest,
    pub(super) server_tx: SharedNetSender<SessionMessage>,
    pub(super) execution_tx: Sender<ExecutionEvent>,
    pub(super) execution_rx: Receiver<ExecutionEvent>,
}

impl ExecutionState {
    pub fn new(
        tool: Tool,
        request: ExecutionRequest,
        server_tx: SharedNetSender<SessionMessage>,
    ) -> Self {
        let (execution_tx, execution_rx) = build_channel();
        Self {
            tool,
            request,
            server_tx,
            execution_tx,
            execution_rx,
        }
    }
}
