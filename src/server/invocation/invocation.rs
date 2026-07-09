use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex as StdMutex};
use std::thread::{self, JoinHandle};

use marix_common::{Logger, Receiver, Sender, build_channel};
use marix_protocol::{
    ExecutionEvent, ExecutionRequest, ExecutionSignature, ExecutionStatus, ExecutorEvent,
    InvocationError, InvocationEvent, InvocationRequest, InvocationSignature, InvocationStatus,
    PlanEvent, SessionEvent, StepEvent, StepSignature, TaskEvent, ToolInputSchema,
};

use crate::task::TaskState;

#[derive(Clone)]
pub struct Invocation {
    pub signature: InvocationSignature,
    pub step: StepSignature,
    invocation_tx: Sender<InvocationEvent>,
    inner: Arc<StdMutex<InvocationInner>>,
    _worker: Arc<StdMutex<Option<JoinHandle<()>>>>,
}

impl Invocation {
    pub fn new(state: Arc<TaskState>, request: InvocationRequest) -> Self {
        let (invocation_tx, invocation_rx) = build_channel();
        let signature = request.signature.clone();
        let step = signature.step.clone();
        let inner = Arc::new(StdMutex::new(InvocationInner::new(request.input)));
        let worker = Arc::new(StdMutex::new(None));
        let invocation = Self {
            signature,
            step,
            invocation_tx,
            inner,
            _worker: Arc::clone(&worker),
        };
        let worker_invocation = invocation.clone();
        let handle = thread::spawn(move || {
            worker_invocation.worker(state, invocation_rx);
        });
        *worker.lock().unwrap_or_else(|error| error.into_inner()) = Some(handle);
        invocation
    }

    pub fn sender(&self) -> Sender<InvocationEvent> {
        self.invocation_tx.clone()
    }

    pub fn status(&self) -> InvocationStatus {
        self.inner
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .status
            .clone()
    }

    pub fn push(&self, seq: usize, content: String) -> bool {
        self.inner
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .push(seq, content)
    }

    pub fn finalize(&self, count: usize) -> bool {
        self.inner
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .finalize(count)
    }

    pub fn content(&self) -> String {
        self.inner
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .content()
    }
}

// -- Private -- //

struct InvocationInner {
    status: InvocationStatus,
    input: ToolInputSchema,
    execution_signature: Option<ExecutionSignature>,
    output: BTreeMap<usize, String>,
    final_signal: Option<usize>,
}

impl InvocationInner {
    fn new(input: ToolInputSchema) -> Self {
        Self {
            status: InvocationStatus::Created,
            input,
            execution_signature: None,
            output: BTreeMap::new(),
            final_signal: None,
        }
    }

    fn push(&mut self, seq: usize, content: String) -> bool {
        self.output.insert(seq, content);
        self.is_complete()
    }

    fn finalize(&mut self, count: usize) -> bool {
        self.final_signal = Some(count);
        self.status = InvocationStatus::Succeed { seq_count: count };
        self.is_complete()
    }

    fn content(&self) -> String {
        self.output.values().cloned().collect()
    }

    fn is_complete(&self) -> bool {
        self.final_signal
            .is_some_and(|count| self.output.len() == count)
    }
}

impl Invocation {
    fn worker(self, state: Arc<TaskState>, invocation_rx: Receiver<InvocationEvent>) {
        Self::send_step_update(&state, &self, InvocationStatus::Created);
        while let Ok(event) = invocation_rx.recv() {
            if let Err(error) = self.dispatch(&state, event) {
                Logger::debug(format!(
                    "invocation {} worker stopping: {error:?}",
                    self.signature.invocation_id.0
                ));
                break;
            }
        }
    }

    fn dispatch(&self, state: &TaskState, event: InvocationEvent) -> Result<(), InvocationError> {
        match event {
            InvocationEvent::Execution(event) => {
                self.forward_execution_event(state, event);
                Ok(())
            }
            InvocationEvent::ExecutionCreate => {
                self.create_execution(state);
                Ok(())
            }
            InvocationEvent::ExecutionUpdate(status) => self.on_execution_update(state, status),
            InvocationEvent::Cancel => {
                self.cancel_execution(state);
                Err(InvocationError::Canceled)
            }
        }
    }

    fn create_execution(&self, state: &TaskState) {
        let (request, status) = {
            let mut inner = self.inner.lock().unwrap_or_else(|error| error.into_inner());
            if inner.execution_signature.is_some() {
                Logger::warning(format!(
                    "invocation {} create ignored: execution already exists",
                    self.signature.invocation_id.0
                ));
                return;
            }
            let execution_signature =
                ExecutionSignature::new(self.signature.clone(), self.signature.name.clone());
            let request = ExecutionRequest {
                signature: execution_signature.clone(),
                input: inner.input.clone(),
            };
            inner.execution_signature = Some(execution_signature);
            inner.status = InvocationStatus::Started;
            (request, inner.status.clone())
        };
        Self::send_step_update(state, self, status);
        Self::send_host_event(
            state,
            ExecutorEvent::ExecutionCreate(request),
            self.signature.invocation_id.0.to_string(),
        );
    }

    fn forward_execution_event(&self, state: &TaskState, event: ExecutionEvent) {
        let execution_signature = self
            .inner
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .execution_signature
            .clone();
        let Some(signature) = execution_signature else {
            Logger::warning(format!(
                "invocation {} could not forward execution event: execution not created",
                self.signature.invocation_id.0
            ));
            return;
        };
        Self::send_host_event(
            state,
            ExecutorEvent::Execution(signature, event),
            self.signature.invocation_id.0.to_string(),
        );
    }

    fn cancel_execution(&self, state: &TaskState) {
        let execution_signature = {
            let mut inner = self.inner.lock().unwrap_or_else(|error| error.into_inner());
            inner.status = InvocationStatus::Canceled;
            inner.execution_signature.clone()
        };
        if let Some(signature) = execution_signature {
            Self::send_host_event(
                state,
                ExecutorEvent::Execution(signature, ExecutionEvent::Cancel),
                self.signature.invocation_id.0.to_string(),
            );
        } else {
            Logger::warning(format!(
                "invocation {} cancel requested before execution create",
                self.signature.invocation_id.0
            ));
        }
        Self::send_step_update(state, self, InvocationStatus::Canceled);
    }

    fn on_execution_update(
        &self,
        state: &TaskState,
        status: ExecutionStatus,
    ) -> Result<(), InvocationError> {
        let invocation_status = Self::map_execution_status(status);
        let error = Self::terminal_status_error(&invocation_status);
        {
            let mut inner = self.inner.lock().unwrap_or_else(|error| error.into_inner());
            match &invocation_status {
                InvocationStatus::Processing { seq, content } => {
                    inner.push(*seq, content.clone());
                    inner.status = invocation_status.clone();
                }
                InvocationStatus::Succeed { seq_count } => {
                    inner.finalize(*seq_count);
                }
                status => {
                    inner.status = status.clone();
                }
            }
        }
        Self::send_step_update(state, self, invocation_status);
        if let Some(error) = error {
            Err(error)
        } else {
            Ok(())
        }
    }

    fn map_execution_status(status: ExecutionStatus) -> InvocationStatus {
        match status {
            ExecutionStatus::Created => InvocationStatus::Created,
            ExecutionStatus::Started => InvocationStatus::Started,
            ExecutionStatus::Processing { seq, content } => {
                InvocationStatus::Processing { seq, content }
            }
            ExecutionStatus::Canceled => InvocationStatus::Canceled,
            ExecutionStatus::Succeed { seq_count } => InvocationStatus::Succeed { seq_count },
            ExecutionStatus::Failed => InvocationStatus::Failed,
        }
    }

    fn terminal_status_error(status: &InvocationStatus) -> Option<InvocationError> {
        match status {
            InvocationStatus::Canceled => Some(InvocationError::ExecutionCanceled),
            InvocationStatus::Succeed { .. } => Some(InvocationError::ExecutionSucceeded),
            InvocationStatus::Failed => Some(InvocationError::ExecutionFailed),
            InvocationStatus::Created
            | InvocationStatus::Started
            | InvocationStatus::Processing { .. } => None,
        }
    }

    fn send_step_update(state: &TaskState, invocation: &Invocation, status: InvocationStatus) {
        let event = SessionEvent::Task(
            invocation.signature.task.clone(),
            TaskEvent::Plan(
                invocation.signature.plan.clone(),
                PlanEvent::Step(invocation.step.clone(), StepEvent::InvocationUpdate(status)),
            ),
        );
        if state.task_tx.send(event).is_err() {
            Logger::warning(format!(
                "invocation {} status update failed: task worker stopped",
                invocation.signature.invocation_id.0
            ));
        }
    }

    fn send_host_event(state: &TaskState, event: ExecutorEvent, invocation_id: String) {
        if state
            .session_tx
            .send(SessionEvent::Executor(event))
            .is_err()
        {
            Logger::warning(format!(
                "invocation {invocation_id} host event failed: session worker stopped"
            ));
        }
    }
}

impl fmt::Debug for Invocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Invocation")
            .field("signature", &self.signature)
            .field("step", &self.step)
            .field("status", &self.status())
            .finish_non_exhaustive()
    }
}
