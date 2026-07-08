use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex as StdMutex};
use std::thread::{self, JoinHandle};

use marix_common::{Logger, Receiver, Sender, build_channel};
use marix_protocol::{
    PlanEvent, RelayError, RelayEvent, RelayRequest, RelaySignature, RelayStatus, SessionEvent,
    StepEvent, StepSignature, TaskEvent,
};

use crate::task::TaskState;

#[derive(Clone)]
pub struct Relay {
    pub signature: RelaySignature,
    pub step: StepSignature,
    relay_tx: Sender<RelayEvent>,
    inner: Arc<StdMutex<RelayInner>>,
    _worker: Arc<StdMutex<Option<JoinHandle<()>>>>,
}

impl Relay {
    pub fn new(state: Arc<TaskState>, request: RelayRequest) -> Self {
        let (relay_tx, relay_rx) = build_channel();
        let signature = request.signature.clone();
        let step = signature.step.clone();
        let inner = Arc::new(StdMutex::new(RelayInner::new(request.prompt)));
        let worker = Arc::new(StdMutex::new(None));
        let relay = Self {
            signature,
            step,
            relay_tx,
            inner,
            _worker: Arc::clone(&worker),
        };
        let worker_relay = relay.clone();
        let handle = thread::spawn(move || {
            worker_relay.worker(state, relay_rx);
        });
        *worker.lock().unwrap_or_else(|error| error.into_inner()) = Some(handle);
        relay
    }

    pub fn sender(&self) -> Sender<RelayEvent> {
        self.relay_tx.clone()
    }

    pub fn status(&self) -> RelayStatus {
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

struct RelayInner {
    status: RelayStatus,
    prompt: String,
    output: BTreeMap<usize, String>,
    final_signal: Option<usize>,
}

impl RelayInner {
    fn new(prompt: String) -> Self {
        Self {
            status: RelayStatus::Created,
            prompt,
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
        self.status = RelayStatus::Succeed { seq_count: count };
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

impl Relay {
    fn worker(self, state: Arc<TaskState>, relay_rx: Receiver<RelayEvent>) {
        Self::send_step_update(&state, &self, RelayStatus::Created);
        let prompt = self
            .inner
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .prompt
            .clone();
        let _ = Logger::warning(format!(
            "relay {} has no execution backend; prompt was not run: {}",
            self.signature.relay_id.0, prompt
        ));
        while let Ok(event) = relay_rx.recv() {
            if let Err(error) = self.dispatch(&state, event) {
                let _ = Logger::debug(format!(
                    "relay {} worker stopping: {error:?}",
                    self.signature.relay_id.0
                ));
                break;
            }
        }
    }

    fn dispatch(&self, state: &TaskState, event: RelayEvent) -> Result<(), RelayError> {
        match event {
            RelayEvent::Cancel => {
                self.cancel(state);
                Err(RelayError::Canceled)
            }
        }
    }

    fn cancel(&self, state: &TaskState) {
        {
            let mut inner = self.inner.lock().unwrap_or_else(|error| error.into_inner());
            inner.status = RelayStatus::Canceled;
        }
        Self::send_step_update(state, self, RelayStatus::Canceled);
    }

    fn send_step_update(state: &TaskState, relay: &Relay, status: RelayStatus) {
        let event = SessionEvent::Task(
            relay.signature.task.clone(),
            TaskEvent::Plan(
                relay.signature.plan.clone(),
                PlanEvent::Step(relay.step.clone(), StepEvent::RelayUpdate(status)),
            ),
        );
        if state.task_tx.send(event).is_err() {
            let _ = Logger::warning(format!(
                "relay {} status update failed: task worker stopped",
                relay.signature.relay_id.0
            ));
        }
    }
}

impl fmt::Debug for Relay {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Relay")
            .field("signature", &self.signature)
            .field("step", &self.step)
            .field("status", &self.status())
            .finish_non_exhaustive()
    }
}
