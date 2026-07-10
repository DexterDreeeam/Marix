use std::convert::Infallible;
use std::sync::Arc;

use marix_common::{Logger, Receiver, Sender, build_channel, select};
use marix_protocol::{
    Actor, ExecutionEvent, ExecutionRequest, ExecutionSignature, ExecutorEvent, Runtime,
};

use super::state::ExecutorState;
use crate::execution::Execution;

pub struct ExecutorRuntime {
    state: Arc<ExecutorState>,
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
}

impl Runtime<ExecutorEvent, Infallible> for ExecutorRuntime {
    fn run(&self) {
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

    fn close(&self) {
        if let Err(error) = self.close_tx.send(()) {
            Logger::warning(format!("host executor close signal failed: {error}"));
        }
    }

    fn dispatch(&self, event: ExecutorEvent) -> Result<(), Infallible> {
        match event {
            ExecutorEvent::Execution(signature, event) => {
                self.dispatch_execution(signature, event);
            }
            ExecutorEvent::ExecutionCreate(request) => {
                self.create_execution(request);
            }
        }
        Ok(())
    }
}

// -- Private -- //

impl ExecutorRuntime {
    fn dispatch_execution(&self, signature: ExecutionSignature, event: ExecutionEvent) {
        let mut event = Some(event);
        match self
            .state
            .executions
            .with(&signature, |execution| {
                execution.dispatch(event.take().unwrap_or_else(|| {
                    unreachable!("execution event already dispatched")
                }))
            })
        {
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
            Logger::warning(format!(
                "execution {} create failed: tool '{}' not found",
                &request.signature, request.signature.name,
            ));
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
            .with_mut(&signature, |execution| execution.start());
    }
}
