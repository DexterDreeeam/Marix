use std::sync::Arc;
use std::thread::{self, JoinHandle};

use crate::executor::Tool;
use crate::session::HostSession;
use marix_common::{Logger, Receiver, Sender, SharedNetSender, build_channel};
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
                    let _ = Logger::log("execution canceled");
                    Self::send_status_event(&state, ExecutionStatus::Canceled);
                    break;
                }
                SessionEvent::Execution(_, ExecutionEvent::Kill) => {
                    let _ = Logger::log("execution killed");
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
            let _ = Logger::debug(format!(
                "executing tool {}",
                state.parameters.signature.name
            ));
            let output = state.tool.execute(&input);
            Self::send_update_event(&state, 0, output);
            Self::send_status_event(&state, ExecutionStatus::Succeed(1));
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

    fn send_update_event(state: &ExecutionState, seq: usize, content: String) {
        Self::send_event(
            state,
            SessionEvent::Execution(
                state.parameters.signature.clone(),
                ExecutionEvent::Update(ExecutionUpdate { seq, content }),
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
