use std::convert::Infallible;
use std::sync::Arc;

use marix_common::{Actor, ActorStatus, Logger, Receiver, Sender, System, build_channel, select};
use marix_protocol::{
    ExecutionEvent, ExecutionRequest, ExecutionResult, ExecutionResultKind, ExecutionSignature,
    ExecutorEvent, InvocationEvent, SessionEvent, TaskEvent,
};

use super::state::ExecutorState;
use crate::execution::Execution;
use crate::session::HostSession;

pub struct ExecutorRuntime {
    state: Arc<ExecutorState>,
    #[allow(dead_code)]
    close_tx: Sender<()>,
    close_rx: Receiver<()>,
}

impl ExecutorRuntime {
    pub fn new(state: Arc<ExecutorState>) -> Self {
        let (close_tx, close_rx) = build_channel();
        Self {
            state,
            close_tx,
            close_rx,
        }
    }

    pub fn run(&self) {
        Logger::debug("host executor runtime loop starting");
        loop {
            select! {
                recv(&self.close_rx) -> _ => {
                    Logger::log("host executor runtime closed");
                    break;
                },
                recv(&self.state.executor_rx) -> event => {
                    let Ok(event) = event else {
                        break;
                    };
                    if let Err(error) = self.dispatch(event) {
                        match error {}
                    }
                }
            }
        }
        Logger::debug("host executor runtime loop stopped");
    }

    #[allow(dead_code)]
    pub fn close(&self) {
        if let Err(error) = self.close_tx.send(()) {
            Logger::warning(format!("host executor close signal failed: {error}"));
        }
    }

    pub fn dispatch(&self, event: ExecutorEvent) -> Result<(), Infallible> {
        match event {
            ExecutorEvent::Execution(signature, event) => {
                self.dispatch_execution(signature, event);
            }
            ExecutorEvent::ExecutionCreate(request) => {
                self.create_execution(request);
            }
            ExecutorEvent::ToolQuery => {
                self.send_executor_tools();
            }
        }
        Ok(())
    }
}

// -- Private -- //

impl ExecutorRuntime {
    fn dispatch_execution(&self, signature: ExecutionSignature, event: ExecutionEvent) {
        let mut event = Some(event);
        match self.state.executions.with(&signature, |execution| {
            execution.dispatch(
                event
                    .take()
                    .unwrap_or_else(|| unreachable!("execution event already dispatched")),
            )
        }) {
            Some(()) => {}
            None => {
                let event = event.unwrap_or_else(|| {
                    unreachable!("execution event dispatched without an execution")
                });
                Logger::warning(format!(
                    "execution {} event {event:?} not routed: execution not found",
                    &signature,
                ));
            }
        }
    }

    fn create_execution(&self, request: ExecutionRequest) {
        let Some(tool) = self.state.registry.get(&request.signature.name).cloned() else {
            let reason = format!("tool '{}' is not available", request.signature.name,);
            Logger::warning(format!(
                "execution {} create failed: {reason}",
                &request.signature,
            ));
            self.send_unknown_tool_failure(&request, reason);
            return;
        };
        let signature = request.signature.clone();
        let execution = Execution::new(tool, request, self.state.server_tx.clone());
        if self
            .state
            .executions
            .insert_or_update(signature.clone(), execution)
        {
            Logger::warning(format!(
                "execution {} replaced existing queue entry",
                &signature,
            ));
        }
        self.state
            .executions
            .with(&signature, |execution| execution.start());
    }

    fn send_executor_tools(&self) {
        let system = System::new();
        let tools = self.state.registry.preview();
        let tool_count = tools.len();
        Logger::debug(format!("executor tools queued with {tool_count} tool(s)"));
        match self.send_server_event(SessionEvent::ExecutorTools(system, tools)) {
            Ok(()) => Logger::debug(format!("executor tools sent with {tool_count} tool(s)")),
            Err(error) => Logger::warning(format!("executor tools send failed: {error}")),
        }
    }

    fn send_unknown_tool_failure(&self, request: &ExecutionRequest, reason: String) {
        let execution = request.signature.clone();
        let invocation = execution.invocation.clone();
        let event = SessionEvent::Task(
            invocation.step.intent.task.clone(),
            TaskEvent::Invocation(
                invocation,
                InvocationEvent::Update(
                    execution,
                    ActorStatus::Complete(ExecutionResult {
                        kind: ExecutionResultKind::Failed,
                        output: reason,
                        seq_count: 0,
                    }),
                ),
            ),
        );
        if let Err(error) = self.send_server_event(event) {
            Logger::warning(format!(
                "execution {} unknown-tool failure could not be sent: \
                 {error}",
                &request.signature,
            ));
        }
    }

    fn send_server_event(&self, event: SessionEvent) -> Result<(), String> {
        let message = HostSession::package_message(event);
        let mut server_tx = self
            .state
            .server_tx
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let Some(sender) = server_tx.as_mut() else {
            return Err("server is disconnected".to_owned());
        };
        sender
            .try_send(message)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}
