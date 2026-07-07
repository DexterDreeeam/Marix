use marix_common::{Logger, SharedNetSender, System, WorkQueue};
use marix_protocol::{
    ExeId, ExecutionEvent, ExecutionRequest, ExecutionSignature, ExecutionStatus, SessionEvent,
    SessionMessage, ToolPreview,
};

use crate::executor::{ExecutionRuntime, ToolRegistry};
use crate::session::HostSession;

pub struct Executor {
    registry: ToolRegistry,
    executions: WorkQueue<ExeId, ExecutionRuntime>,
    server_tx: SharedNetSender<SessionMessage>,
}

impl Executor {
    pub fn new(server_tx: SharedNetSender<SessionMessage>) -> Self {
        Self {
            registry: ToolRegistry::new(),
            executions: WorkQueue::new(),
            server_tx,
        }
    }

    pub fn preview(&self) -> Vec<ToolPreview> {
        self.registry.preview()
    }

    pub fn route_session_event(&mut self, event: SessionEvent) {
        let SessionEvent::Execution(signature, execution_event) = event else {
            return;
        };
        match execution_event {
            ExecutionEvent::PreviewQuery => self.send_preview_event(signature),
            ExecutionEvent::Evoke(request) => self.create_execution(request),
            execution_event => self.forward_to_execution(signature, execution_event),
        }
    }
}

// -- Private -- //

impl Executor {
    fn create_execution(&mut self, request: ExecutionRequest) {
        let tool = match self.registry.get(&request.signature.name) {
            Some(tool) => tool.clone(),
            None => {
                let _ = Logger::error(format!(
                    "unknown tool requested: {}",
                    request.signature.name
                ));
                let reason = format!("unknown tool: {}", request.signature.name);
                self.send_failed_event(&request.signature, reason);
                return;
            }
        };
        let exe_id = request.signature.exe_id.clone();
        let _ = Logger::log(format!(
            "starting execution {} for tool {}",
            exe_id.0, request.signature.name
        ));
        let runtime = ExecutionRuntime::new(tool, request, self.server_tx.clone());
        self.executions.insert(exe_id, runtime);
    }

    fn forward_to_execution(&self, signature: ExecutionSignature, execution_event: ExecutionEvent) {
        let sender = self
            .executions
            .with(&signature.exe_id, |runtime| runtime.sender());
        if let Some(sender) = sender {
            let _ = sender.send(SessionEvent::Execution(signature, execution_event));
        }
    }

    fn send_failed_event(&self, signature: &ExecutionSignature, reason: String) {
        if let Some(sender) = self
            .server_tx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_mut()
        {
            let _ = sender.try_send(HostSession::package_message(SessionEvent::Execution(
                signature.clone(),
                ExecutionEvent::Status(ExecutionStatus::Failed { reason }),
            )));
        }
    }

    fn send_preview_event(&self, signature: ExecutionSignature) {
        if let Some(sender) = self
            .server_tx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_mut()
        {
            let _ = sender.try_send(HostSession::package_message(SessionEvent::Execution(
                signature,
                ExecutionEvent::Preview {
                    system: System::new(),
                    tools: self.preview(),
                },
            )));
        }
    }
}
