use crate::execution::Execution;
use crate::executor::Tool;
use crate::session::HostSession;
use marix_common::{
    ActorStartFuture, ActorStatus, Lifecycle, Logger, Runtime as RuntimeTrait, SharedNetSender,
};
use marix_protocol::{
    ExecutionEvent, ExecutionRequest, ExecutionResult, ExecutionResultKind, ExecutionSignature,
    InvocationEvent, SessionEvent, SessionMessage, TaskEvent,
};

pub struct ExecutionRuntime {
    pub lifecycle: Lifecycle<ExecutionEvent, ExecutionResult>,
    pub(super) tool: Tool,
    pub(super) request: ExecutionRequest,
    pub(super) server_tx: SharedNetSender<SessionMessage>,
}

impl ExecutionRuntime {
    pub fn new(
        tool: Tool,
        request: ExecutionRequest,
        server_tx: SharedNetSender<SessionMessage>,
    ) -> Self {
        Self {
            lifecycle: Lifecycle::new(),
            tool,
            request,
            server_tx,
        }
    }
}

impl RuntimeTrait for ExecutionRuntime {
    type Base = Execution;
    type Prepared = ();

    fn signature(&self) -> &ExecutionSignature {
        &self.request.signature
    }

    fn lifecycle(&self) -> &Lifecycle<ExecutionEvent, ExecutionResult> {
        &self.lifecycle
    }

    fn on_start(&self) -> ActorStartFuture<'_, Self::Prepared> {
        Box::pin(async move {
            self.send_status(ActorStatus::Running);
            Logger::log(format!("execution {} started", &self.request.signature,));
            let content = self.tool.execute(&self.request.input);
            self.send_processing(0, content);
            RuntimeTrait::finish(
                self,
                ExecutionResult {
                    kind: ExecutionResultKind::Succeed,
                    output: String::new(),
                    seq_count: 1,
                },
            );
            None
        })
    }

    fn dispatch(&self, event: ExecutionEvent) {
        match event {
            ExecutionEvent::Cancel => {
                if matches!(self.status(), ActorStatus::Complete(_)) {
                    return;
                }
                let reason = format!("execution {} canceled", &self.request.signature);
                Logger::log(&reason);
                RuntimeTrait::finish(
                    self,
                    ExecutionResult {
                        kind: ExecutionResultKind::Canceled,
                        output: reason,
                        seq_count: 0,
                    },
                );
            }
        }
    }

    fn on_finish(&self, result: ExecutionResult) {
        self.send_status(ActorStatus::Complete(result));
    }
}

// -- Private -- //

impl ExecutionRuntime {
    fn send_status(&self, status: ActorStatus<ExecutionResult>) {
        let execution = self.request.signature.clone();
        let invocation = execution.invocation.clone();
        let event = SessionEvent::Task(
            invocation.step.intent.task.clone(),
            TaskEvent::Invocation(invocation, InvocationEvent::Update(execution, status)),
        );
        self.send_server_event(event);
    }

    fn send_processing(&self, seq: usize, content: String) {
        let execution = self.request.signature.clone();
        let invocation = execution.invocation.clone();
        let event = SessionEvent::Task(
            invocation.step.intent.task.clone(),
            TaskEvent::Invocation(
                invocation,
                InvocationEvent::Processing {
                    execution,
                    seq,
                    content,
                },
            ),
        );
        self.send_server_event(event);
    }

    fn send_server_event(&self, event: SessionEvent) {
        let message = HostSession::package_message(event);
        let mut server_tx = self
            .server_tx
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let Some(sender) = server_tx.as_mut() else {
            Logger::warning(format!(
                "execution {} could not send event: server is disconnected",
                &self.request.signature,
            ));
            return;
        };
        if let Err(error) = sender.try_send(message) {
            Logger::warning(format!(
                "execution {} could not send event: {}",
                &self.request.signature, error,
            ));
        }
    }
}
