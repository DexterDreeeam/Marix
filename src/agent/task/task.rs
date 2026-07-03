use std::thread::JoinHandle;

use marix_common::{
    ExeId, Receiver, Sender, SessionEvent, SharedNetSender, TaskId, TaskSignature, WorkQueue,
    channel,
};

use crate::model::ModelBackend;
use crate::task::{Execution, Step, StepSequence};

pub struct Task {
    id: TaskId,
    signature: TaskSignature,
    model_backend: Option<Box<dyn ModelBackend>>,
    client_tx: SharedNetSender<SessionEvent>,
    host_tx: SharedNetSender<SessionEvent>,
    task_tx: Sender<SessionEvent>,
    task_rx: Option<Receiver<SessionEvent>>,
    executions: WorkQueue<ExeId, Execution>,
    steps: WorkQueue<StepSequence, Step>,
    worker: Option<JoinHandle<()>>,
}

impl Task {
    pub fn new(
        signature: TaskSignature,
        client_tx: SharedNetSender<SessionEvent>,
        host_tx: SharedNetSender<SessionEvent>,
    ) -> Self {
        let (task_tx, task_rx) = channel();
        Self {
            id: signature.id.clone(),
            signature,
            model_backend: None,
            client_tx,
            host_tx,
            task_tx,
            task_rx: Some(task_rx),
            executions: WorkQueue::new(),
            steps: WorkQueue::new(),
            worker: None,
        }
    }

    pub fn sender(&self) -> Sender<SessionEvent> {
        self.task_tx.clone()
    }

    pub fn run(&mut self) {
        if self.worker.is_some() {
            return;
        }
        let Some(task_rx) = self.task_rx.take() else {
            panic!("task receiver is missing")
        };
        self.worker = Some(std::thread::spawn(move || while task_rx.recv().is_ok() {}));
    }

    pub fn raise(&self, step: Step) {
        self.steps.insert(step.sequence, step);
    }
}
