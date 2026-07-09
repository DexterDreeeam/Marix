use std::fmt;
use std::sync::{Arc, Mutex as StdMutex};
use std::thread::{self, JoinHandle};

use marix_common::{Logger, Receiver, Sender, build_channel};
use marix_protocol::{
    PlanDraft, PlanError, PlanEvent, PlanSignature, PlanStatus, StepDraft, StepEvent,
    StepSignature, StepStatus, TaskEvent,
};

use crate::step::Step;
use crate::task::TaskState;

#[derive(Clone)]
pub struct Plan {
    pub state: Arc<TaskState>,
    pub signature: PlanSignature,
    pub description: String,
    pub run_steps: Vec<Step>,
    pub pending_steps: Vec<Step>,
    pub expected_result: String,
    plan_tx: Sender<PlanEvent>,
    _worker: Arc<StdMutex<Option<JoinHandle<()>>>>,
}

impl Plan {
    pub(crate) fn from_draft(
        state: &Arc<TaskState>,
        signature: PlanSignature,
        draft: PlanDraft,
    ) -> Result<Self, PlanError> {
        let run_steps = Self::build_steps(state, &signature, draft.run_steps)?;
        let pending_steps = Self::build_steps(state, &signature, draft.pending_steps)?;
        let (plan_tx, plan_rx) = build_channel();
        let worker = Arc::new(StdMutex::new(None));
        let plan = Self {
            state: Arc::clone(state),
            signature,
            description: draft.description,
            run_steps,
            pending_steps,
            expected_result: draft.expected_result,
            plan_tx,
            _worker: Arc::clone(&worker),
        };
        let worker_plan = plan.clone();
        let handle = thread::spawn(move || worker_plan.worker(plan_rx));
        *worker.lock().unwrap_or_else(|error| error.into_inner()) = Some(handle);
        Ok(plan)
    }

    pub(crate) fn run_step_signatures(&self) -> Vec<StepSignature> {
        self.run_steps
            .iter()
            .map(|step| step.signature.clone())
            .collect()
    }

    pub(crate) fn step(&self, signature: &StepSignature) -> Option<Step> {
        self.run_steps
            .iter()
            .chain(self.pending_steps.iter())
            .find(|step| &step.signature == signature)
            .cloned()
    }

    pub(crate) fn sender(&self) -> Sender<PlanEvent> {
        self.plan_tx.clone()
    }

    pub(crate) fn start_run_steps(&self) {
        if self.run_steps.is_empty() {
            self.send_task_event(TaskEvent::PlanUpdate(PlanStatus::Success));
            return;
        }
        for step in &self.run_steps {
            step.start();
        }
    }
}

// -- Private -- //

impl Plan {
    fn worker(self, plan_rx: Receiver<PlanEvent>) {
        while let Ok(event) = plan_rx.recv() {
            if let Err(error) = self.dispatch(event) {
                Logger::debug(format!(
                    "plan {} worker stopping: {error:?} (task {})",
                    self.signature.id.0, self.signature.task.id.0
                ));
                break;
            }
        }
    }

    fn dispatch(&self, event: PlanEvent) -> Result<(), PlanError> {
        match event {
            PlanEvent::Step(signature, event) => {
                self.dispatch_step(signature, event);
                Ok(())
            }
            PlanEvent::StepCreate(draft) => {
                Logger::warning(format!(
                    "plan {} step create '{}' ignored: dynamic step insertion is not supported",
                    self.signature.id.0, draft.name
                ));
                Ok(())
            }
            PlanEvent::StepUpdate(status) => self.on_step_update(status),
            PlanEvent::Cancel => {
                self.cancel_steps();
                self.send_task_event(TaskEvent::PlanUpdate(PlanStatus::Fail));
                Err(PlanError::Canceled)
            }
        }
    }

    fn on_step_update(&self, status: StepStatus) -> Result<(), PlanError> {
        match status {
            StepStatus::Succeed => {
                self.send_task_event(TaskEvent::PlanUpdate(PlanStatus::Success));
                Ok(())
            }
            StepStatus::Canceled | StepStatus::Failed => {
                self.send_task_event(TaskEvent::PlanUpdate(PlanStatus::Fail));
                Ok(())
            }
            StepStatus::Created | StepStatus::Started => Ok(()),
        }
    }

    fn build_steps(
        state: &Arc<TaskState>,
        signature: &PlanSignature,
        drafts: Vec<StepDraft>,
    ) -> Result<Vec<Step>, PlanError> {
        drafts
            .into_iter()
            .map(|draft| Step::from_draft(state, signature, draft))
            .collect()
    }

    fn dispatch_step(&self, signature: StepSignature, event: StepEvent) {
        let event_name = format!("{event:?}");
        let Some(step) = self.step(&signature) else {
            Logger::error(format!(
                "step {} event {event_name} not dispatched: step not found (task {})",
                signature.id.0, signature.task.id.0
            ));
            return;
        };
        if step.sender().send(event).is_err() {
            Logger::warning(format!(
                "step {} event {event_name} dispatch failed: worker stopped (task {})",
                signature.id.0, signature.task.id.0
            ));
        }
    }

    fn cancel_steps(&self) {
        for step in &self.run_steps {
            if step.sender().send(StepEvent::Cancel).is_err() {
                Logger::warning(format!(
                    "step {} cancel failed: worker stopped (task {})",
                    step.signature.id.0, step.signature.task.id.0
                ));
            }
        }
    }

    fn send_task_event(&self, event: TaskEvent) {
        if self
            .state
            .task_tx
            .send(marix_protocol::SessionEvent::Task(
                self.signature.task.clone(),
                event,
            ))
            .is_err()
        {
            Logger::warning(format!(
                "plan {} update failed: task worker stopped (task {})",
                self.signature.id.0, self.signature.task.id.0
            ));
        }
    }
}

impl fmt::Debug for Plan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let run_steps = self
            .run_steps
            .iter()
            .map(|step| &step.signature)
            .collect::<Vec<_>>();
        let pending_steps = self
            .pending_steps
            .iter()
            .map(|step| &step.signature)
            .collect::<Vec<_>>();
        formatter
            .debug_struct("Plan")
            .field("signature", &self.signature)
            .field("description", &self.description)
            .field("run_steps", &run_steps)
            .field("pending_steps", &pending_steps)
            .field("expected_result", &self.expected_result)
            .finish()
    }
}
