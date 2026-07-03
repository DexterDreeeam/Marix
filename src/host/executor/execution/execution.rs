use std::sync::Arc;
use std::thread::{self, JoinHandle};

use crate::executor::Tool;
use marix_common::{
    ExecutionEvent, ExecutionRequest, ExecutionStatus, ExecutionUpdate, Receiver, Sender,
    SessionEvent, SharedNetSender, build_channel,
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
        agent_tx: SharedNetSender<SessionEvent>,
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
        Self::emit_status(&context, ExecutionStatus::Started);
        Self::spawn_execution(Arc::clone(&context));
        while let Ok(event) = execution_rx.recv() {
            match event {
                SessionEvent::Execution(_, ExecutionEvent::Cancel) => {
                    Self::emit_status(&context, ExecutionStatus::Canceled);
                    break;
                }
                SessionEvent::Execution(_, ExecutionEvent::Kill) => {
                    Self::emit_status(&context, ExecutionStatus::Killed);
                    break;
                }
                _ => {}
            }
        }
    }

    fn spawn_execution(context: Arc<ExecutionContext>) {
        thread::spawn(move || {
            let input = context.parameters.tool_request.clone().unwrap_or_default();
            let output = context.tool.execute(&input);
            Self::emit_update(&context, output);
            Self::emit_status(&context, ExecutionStatus::Succeed);
        });
    }

    fn emit_status(context: &ExecutionContext, status: ExecutionStatus) {
        Self::send_event(
            context,
            SessionEvent::Execution(
                context.parameters.signature.clone(),
                ExecutionEvent::Status(status),
            ),
        );
    }

    fn emit_update(context: &ExecutionContext, content: String) {
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
            let _ = sender.try_send(event);
        }
    }
}
