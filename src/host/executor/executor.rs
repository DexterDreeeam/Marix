use std::sync::Arc;
use std::thread::{self, JoinHandle};

use marix_common::{Logger, Receiver, Sender, SharedNetSender, build_channel};
use marix_protocol::{
    ExecutionEvent, ExecutionRequest, ExecutionSignature, ExecutorEvent, InvocationEvent,
    PlanEvent, SessionEvent, SessionMessage, StepEvent, TaskEvent, ToolPreview,
};

use super::state::ExecutorState;
use crate::executor::Execution;
use crate::session::HostSession;

pub struct Executor {
    executor_tx: Sender<ExecutorEvent>,
    state: Arc<ExecutorState>,
    _worker: JoinHandle<()>,
}

impl Executor {
    pub fn new(server_tx: SharedNetSender<SessionMessage>) -> Self {
        let (executor_tx, executor_rx) = build_channel();
        let state = Arc::new(ExecutorState::new(executor_tx.clone()));
        let worker = thread::spawn({
            let state = Arc::clone(&state);
            let server_tx = server_tx.clone();
            move || Self::worker(state, server_tx, executor_rx)
        });
        Self {
            executor_tx,
            state,
            _worker: worker,
        }
    }

    pub fn preview(&self) -> Vec<ToolPreview> {
        self.state.registry.preview()
    }

    pub fn sender(&self) -> Sender<ExecutorEvent> {
        self.executor_tx.clone()
    }
}

// -- Private -- //

impl Executor {
    fn dispatch(
        state: &ExecutorState,
        server_tx: &SharedNetSender<SessionMessage>,
        event: ExecutorEvent,
    ) {
        match event {
            ExecutorEvent::Execution(signature, event) => {
                Self::dispatch_execution(state, signature, event);
            }
            ExecutorEvent::ExecutionCreate(request) => {
                Self::create_execution(state, request);
            }
            ExecutorEvent::ExecutionUpdate(signature, status) => {
                let invocation = signature.invocation;
                let event = SessionEvent::Task(
                    invocation.task.clone(),
                    TaskEvent::Plan(
                        invocation.plan.clone(),
                        PlanEvent::Step(
                            invocation.step.clone(),
                            StepEvent::Invocation(
                                invocation,
                                InvocationEvent::ExecutionUpdate(status),
                            ),
                        ),
                    ),
                );
                Self::send_server_event(server_tx, event);
            }
        }
    }

    fn dispatch_execution(
        state: &ExecutorState,
        signature: ExecutionSignature,
        event: ExecutionEvent,
    ) {
        let event_name = format!("{event:?}");
        let execution_id = signature.execution_id.0;
        match state.executions.with(&signature, |execution| {
            execution.sender().send(event).is_ok()
        }) {
            Some(true) => {}
            Some(false) => {
                let _ = Logger::warning(format!(
                    "execution {} event {event_name} forward failed: worker stopped",
                    execution_id
                ));
            }
            None => {
                let _ = Logger::warning(format!(
                    "execution {} event {event_name} not routed: execution not found",
                    execution_id
                ));
            }
        }
    }

    fn create_execution(state: &ExecutorState, request: ExecutionRequest) {
        let Some(tool) = state.registry.get(&request.signature.name).cloned() else {
            let _ = Logger::warning(format!(
                "execution {} create failed: tool '{}' not found",
                request.signature.execution_id.0, request.signature.name
            ));
            return;
        };
        let signature = request.signature.clone();
        let execution = Execution::new(tool, request, state.executor_tx.clone());
        if state
            .executions
            .insert_or_update(signature.clone(), execution)
        {
            let _ = Logger::warning(format!(
                "execution {} replaced existing queue entry",
                signature.execution_id.0
            ));
        }
    }

    fn worker(
        state: Arc<ExecutorState>,
        server_tx: SharedNetSender<SessionMessage>,
        executor_rx: Receiver<ExecutorEvent>,
    ) {
        while let Ok(event) = executor_rx.recv() {
            Self::dispatch(&state, &server_tx, event);
        }
    }

    fn send_server_event(server_tx: &SharedNetSender<SessionMessage>, event: SessionEvent) {
        let message = HostSession::package_message(event);
        let mut server_tx = server_tx.lock().unwrap_or_else(|error| error.into_inner());
        let Some(sender) = server_tx.as_mut() else {
            let _ = Logger::warning(
                "host executor worker could not send event: server is disconnected",
            );
            return;
        };
        if let Err(error) = sender.try_send(message) {
            let _ = Logger::warning(format!(
                "host executor worker could not send event: {}",
                error
            ));
        }
    }
}
