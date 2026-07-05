use std::sync::Arc;
use std::thread::{self, JoinHandle};

use crate::executor::Tool;
use crate::session::HostSession;
use marix_common::{Receiver, Sender, SharedNetSender, build_channel};
use marix_protocol::{
    ExecutionEvent, ExecutionRequest, ExecutionStatus, ExecutionUpdate, SessionEvent,
    SessionMessage,
};

use super::ExecutionContext;

pub struct ExecutionRuntime {
    context: Arc<ExecutionContext>,
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
        let context = Arc::new(ExecutionContext::new(tool, parameters, agent_tx));
        let worker = thread::spawn({
            let context = Arc::clone(&context);
            move || Self::event_loop(context, execution_rx)
        });
        Self {
            context,
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
    fn event_loop(context: Arc<ExecutionContext>, execution_rx: Receiver<SessionEvent>) {
        Self::send_status_event(&context, ExecutionStatus::Started);
        Self::spawn_execution(Arc::clone(&context));
        while let Ok(event) = execution_rx.recv() {
            match event {
                SessionEvent::Execution(_, ExecutionEvent::Cancel) => {
                    Self::send_status_event(&context, ExecutionStatus::Canceled);
                    break;
                }
                SessionEvent::Execution(_, ExecutionEvent::Kill) => {
                    Self::send_status_event(&context, ExecutionStatus::Killed);
                    break;
                }
                _ => {}
            }
        }
    }

    fn spawn_execution(context: Arc<ExecutionContext>) {
        thread::spawn(move || {
            let input = context.parameters.input.content.clone();
            let output = context.tool.execute(&input);
            Self::send_update_event(&context, output);
            Self::send_status_event(&context, ExecutionStatus::Succeed);
        });
    }

    fn send_status_event(context: &ExecutionContext, status: ExecutionStatus) {
        Self::send_event(
            context,
            SessionEvent::Execution(
                context.parameters.signature.clone(),
                ExecutionEvent::Status(status),
            ),
        );
    }

    fn send_update_event(context: &ExecutionContext, content: String) {
        Self::send_event(
            context,
            SessionEvent::Execution(
                context.parameters.signature.clone(),
                ExecutionEvent::Update(ExecutionUpdate { content }),
            ),
        );
    }

    fn send_event(context: &ExecutionContext, event: SessionEvent) {
        if let Some(sender) = context
            .agent_tx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_mut()
        {
            let _ = sender.try_send(HostSession::package_message(event));
        }
    }
}
