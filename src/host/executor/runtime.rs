use std::sync::Arc;

use marix_common::{Actor, ActorStatus, Logger, Receiver, Sender, System, build_channel, select};
use marix_protocol::{
    ContinuationRequest, ExecutionEvent, ExecutionRequest,
    ExecutionResult, ExecutionResultKind, ExecutionSignature,
    ExecutorEvent, InvocationEvent, InvocationSignature, SessionEvent,
    TaskEvent, TaskLogger,
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
                    Logger::info("host executor runtime closed");
                    break;
                },
                recv(&self.state.executor_rx) -> event => {
                    let Ok(event) = event else {
                        break;
                    };
                    self.dispatch(event);
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

    pub fn dispatch(&self, event: ExecutorEvent) {
        match event {
            ExecutorEvent::Continuation(request) => {
                self.continue_invocation(request);
            }
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
    }
}

// -- Private -- //

impl ExecutorRuntime {
    fn dispatch_execution(&self, signature: ExecutionSignature, event: ExecutionEvent) {
        let logger = TaskLogger::from(signature.invocation.step.intent.task.clone());
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
                logger.warning(format!(
                    "execution {} event {event:?} not routed: execution not found",
                    &signature,
                ));
            }
        }
    }

    fn create_execution(&self, request: ExecutionRequest) {
        let logger = TaskLogger::from(
            request
                .signature
                .invocation
                .step
                .intent
                .task
                .clone(),
        );
        let Some(tool) = self.state.registry.get(&request.signature.name).cloned() else {
            let reason = format!("tool '{}' is not available", request.signature.name,);
            logger.warning(format!(
                "execution {} create failed: {reason}",
                &request.signature,
            ));
            self.send_execution_failure(&request, reason);
            return;
        };
        let signature = request.signature.clone();
        let execution = Execution::new(
            tool,
            request,
            self.state.server_tx.clone(),
            self.state.cache.clone(),
        );
        if self
            .state
            .executions
            .insert_or_update(signature.clone(), execution)
        {
            logger.warning(format!(
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

    fn continue_invocation(&self, request: ContinuationRequest) {
        let result = {
            let mut cache = self
                .state
                .cache
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            cache.pick(&request.continuation_cursor)
        };
        match result {
            Ok((content, continuation_cursor)) => {
                self.send_invocation_event(
                    &request.invocation,
                    InvocationEvent::Continuation {
                        content,
                        continuation_cursor,
                    },
                );
            }
            Err(reason) => {
                self.send_invocation_event(
                    &request.invocation,
                    InvocationEvent::ContinuationFailed { reason },
                );
            }
        }
    }

    fn send_execution_failure(&self, request: &ExecutionRequest, reason: String) {
        self.send_invocation_event(
            &request.signature.invocation,
            InvocationEvent::Update(
                request.signature.clone(),
                ActorStatus::Complete(ExecutionResult {
                    kind: ExecutionResultKind::Failed,
                    output: reason,
                    seq_count: 0,
                    continuation_cursor: None,
                }),
            ),
        );
    }

    fn send_invocation_event(
        &self,
        invocation: &InvocationSignature,
        invocation_event: InvocationEvent,
    ) {
        let task = invocation.step.intent.task.clone();
        let logger = TaskLogger::from(task.clone());
        let event = SessionEvent::Task(
            task,
            TaskEvent::Invocation(
                invocation.clone(),
                invocation_event,
            ),
        );
        if let Err(error) = self.send_server_event(event) {
            logger.warning(format!(
                "invocation {invocation} event could not be sent: \
                 {error}",
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
