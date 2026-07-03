use marix_common::{
    ExeId, ExecutionEvent, ExecutionRequest, ExecutionSignature, ExecutionStatus, SessionEvent,
    SharedNetSender, ToolPreview, WorkQueue,
};

use crate::executor::{ExecutionRuntime, ToolRegistry};

pub struct Executor {
    registry: ToolRegistry,
    executions: WorkQueue<ExeId, ExecutionRuntime>,
    agent_tx: SharedNetSender<SessionEvent>,
}

impl Executor {
    pub fn new(agent_tx: SharedNetSender<SessionEvent>) -> Self {
        Self {
            registry: ToolRegistry::new(),
            executions: WorkQueue::new(),
            agent_tx,
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
                let reason = format!("unknown tool: {}", request.signature.name);
                self.emit_failed(&request.signature, reason);
                return;
            }
        };
        let exe_id = request.signature.exe_id.clone();
        let runtime = ExecutionRuntime::new(tool, request, self.agent_tx.clone());
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

    fn emit_failed(&self, signature: &ExecutionSignature, reason: String) {
        if let Some(sender) = self
            .agent_tx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_mut()
        {
            let _ = sender.try_send(SessionEvent::Execution(
                signature.clone(),
                ExecutionEvent::Status(ExecutionStatus::Failed { reason }),
            ));
        }
    }
}
