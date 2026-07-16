use std::sync::Arc;

use crate::execution::ExecutionState;
use crate::session::HostSession;
use marix_common::{Logger, Receiver, Sender, build_channel, select};
use marix_protocol::{
    ExecutionError, ExecutionEvent, ExecutionStatus, InvocationEvent, Runtime, SessionEvent,
    TaskEvent,
};

pub struct ExecutionRuntime {
    pub(super) state: Arc<ExecutionState>,
    pub close_tx: Sender<()>,
    pub close_rx: Receiver<()>,
}

impl ExecutionRuntime {
    pub fn new(state: Arc<ExecutionState>) -> Self {
        let (close_tx, close_rx) = build_channel();
        Self {
            state,
            close_tx,
            close_rx,
        }
    }
}

impl Runtime<ExecutionEvent, ExecutionError> for ExecutionRuntime {
    fn run(&self) {
        self.execute();
        Logger::debug(format!(
            "execution {} runtime loop starting",
            &self.state.request.signature,
        ));
        loop {
            select! {
                recv(&self.close_rx) -> _ => {
                    Logger::log(format!(
                        "execution {} closed",
                        &self.state.request.signature,
                    ));
                    break;
                },
                recv(&self.state.execution_rx) -> event => {
                    let Ok(event) = event else {
                        break;
                    };
                    if let Err(error) = self.dispatch(event) {
                        Logger::debug(format!(
                            "execution {} runtime stopping: {error:?}",
                            &self.state.request.signature,
                        ));
                        break;
                    }
                }
            }
        }
        Logger::debug(format!(
            "execution {} runtime loop stopped",
            &self.state.request.signature,
        ));
    }

    fn close(&self) {
        if let Err(error) = self.close_tx.send(()) {
            Logger::warning(format!(
                "execution {} close signal failed: {}",
                &self.state.request.signature, error,
            ));
        }
    }

    fn dispatch(&self, event: ExecutionEvent) -> Result<(), ExecutionError> {
        match event {
            ExecutionEvent::Cancel => {
                self.on_cancel();
                Err(ExecutionError::Canceled)
            }
        }
    }
}

// -- Private -- //

impl ExecutionRuntime {
    pub(super) fn execute(&self) {
        self.send_status(ExecutionStatus::Created);
        self.send_status(ExecutionStatus::Started);
        Logger::log(format!(
            "execution {} started",
            &self.state.request.signature,
        ));
        let content = self.state.tool.execute(&self.state.request.input);
        self.send_processing(0, content);
        self.send_status(ExecutionStatus::Succeed { seq_count: 1 });
        self.close();
    }

    pub(super) fn send_status(&self, status: ExecutionStatus) {
        let execution = self.state.request.signature.clone();
        let invocation = execution.invocation.clone();
        let event = SessionEvent::Task(
            invocation.step.intent.task.clone(),
            TaskEvent::Invocation(invocation, InvocationEvent::Update(execution, status)),
        );
        self.send_server_event(event);
    }

    pub(super) fn send_processing(&self, seq: usize, content: String) {
        let execution = self.state.request.signature.clone();
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

    fn on_cancel(&self) {
        Logger::log(format!(
            "execution canceled for tool {}",
            self.state.tool.name(),
        ));
        self.send_status(ExecutionStatus::Canceled);
        self.close();
    }

    fn send_server_event(&self, event: SessionEvent) {
        let message = HostSession::package_message(event);
        let mut server_tx = self
            .state
            .server_tx
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let Some(sender) = server_tx.as_mut() else {
            Logger::warning(format!(
                "execution {} could not send event: server is disconnected",
                &self.state.request.signature,
            ));
            return;
        };
        if let Err(error) = sender.try_send(message) {
            Logger::warning(format!(
                "execution {} could not send event: {}",
                &self.state.request.signature, error,
            ));
        }
    }
}
