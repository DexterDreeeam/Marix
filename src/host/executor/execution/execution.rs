use std::sync::Arc;
use std::thread::{self, JoinHandle};

use crate::executor::Tool;
use crate::session::HostSession;
use marix_common::{Receiver, Sender, SharedNetSender, build_channel};
use marix_protocol::{
    ExecutionEvent, ExecutionRequest, ExecutionStatus, ExecutionUpdate, SessionEvent,
    SessionMessage,
};

use super::ExecutionState;

pub struct ExecutionRuntime {
    state: Arc<ExecutionState>,
    execution_tx: Sender<SessionEvent>,
    worker: JoinHandle<()>,
}

impl ExecutionRuntime {
    pub fn new(
        tool: Tool,
        parameters: ExecutionRequest,
        agent_tx: SharedNetSender<SessionMessage>,
    ) -> Self {
        let (execution_tx, execution_rx) = build_channel();
        let state = Arc::new(ExecutionState::new(tool, parameters, agent_tx));
        let worker = thread::spawn({
            let state = Arc::clone(&state);
            move || Self::event_loop(state, execution_rx)
        });
        Self {
            state,
            execution_tx,
            worker,
        }
    }

    pub fn sender(&self) -> Sender<SessionEvent> {
        self.execution_tx.clone()
    }
}

// -- Private -- //

impl ExecutionRuntime {
    fn event_loop(state: Arc<ExecutionState>, execution_rx: Receiver<SessionEvent>) {
        Self::send_status_event(&state, ExecutionStatus::Started);
        Self::spawn_execution(Arc::clone(&state));
        while let Ok(event) = execution_rx.recv() {
            match event {
                SessionEvent::Execution(_, ExecutionEvent::Cancel) => {
                    Self::send_status_event(&state, ExecutionStatus::Canceled);
                    break;
                }
                SessionEvent::Execution(_, ExecutionEvent::Kill) => {
                    Self::send_status_event(&state, ExecutionStatus::Killed);
                    break;
                }
                _ => {}
            }
        }
    }

    fn spawn_execution(state: Arc<ExecutionState>) {
        thread::spawn(move || {
            let input = state.parameters.input.content.clone();
            let output = state.tool.execute(&input);
            Self::send_update_event(&state, output);
            Self::send_status_event(&state, ExecutionStatus::Succeed);
        });
    }

    fn send_status_event(state: &ExecutionState, status: ExecutionStatus) {
        Self::send_event(
            state,
            SessionEvent::Execution(
                state.parameters.signature.clone(),
                ExecutionEvent::Status(status),
            ),
        );
    }

    fn send_update_event(state: &ExecutionState, content: String) {
        Self::send_event(
            state,
            SessionEvent::Execution(
                state.parameters.signature.clone(),
                ExecutionEvent::Update(ExecutionUpdate { content }),
            ),
        );
    }

    fn send_event(state: &ExecutionState, event: SessionEvent) {
        if let Some(sender) = state
            .agent_tx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_mut()
        {
            let _ = sender.try_send(HostSession::package_message(event));
        }
    }
}
