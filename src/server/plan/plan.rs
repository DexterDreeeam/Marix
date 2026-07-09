use std::fmt;

use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::external::*;
use marix_common::{Logger, build_async_channel};
use marix_protocol::{
    PlanDraft, PlanError, PlanEvent, PlanSignature, PlanStatus, StepDraft, StepEvent,
    StepSignature, StepStatus, TaskEvent,
};

use crate::step::Step;
use crate::task::TaskAccess;

pub struct Plan {
    state: Arc<PlanState>,
    worker_started: bool,
}

impl Clone for Plan {
    fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
            worker_started: false,
        }
    }
}

impl Plan {
    pub(crate) fn from_draft(
        access: TaskAccess,
        signature: PlanSignature,
        draft: PlanDraft,
    ) -> Result<Self, PlanError> {
        let run_steps = Self::build_steps(access.clone(), &signature, draft.run_steps)?;
        let pending_steps = Self::build_steps(access.clone(), &signature, draft.pending_steps)?;
        let plan = Self {
            state: Arc::new(PlanState::new(
                access,
                signature,
                draft.description,
                run_steps,
                pending_steps,
                draft.expected_result,
            )),
            worker_started: false,
        };
        Ok(plan)
    }

    pub(crate) fn run_step_signatures(&self) -> Vec<StepSignature> {
        self.state
            .run_steps
            .iter()
            .map(|step| step.signature().clone())
            .collect()
    }

    pub(crate) fn step(&self, signature: &StepSignature) -> Option<Step> {
        self.state
            .run_steps
            .iter()
            .chain(self.state.pending_steps.iter())
            .find(|step| step.signature() == signature)
            .cloned()
    }

    pub(crate) fn sender(&self) -> tokio::mpsc::UnboundedSender<PlanEvent> {
        self.state.plan_tx.clone()
    }

    pub(crate) fn signature(&self) -> &PlanSignature {
        &self.state.signature
    }

    pub(crate) fn description(&self) -> &str {
        &self.state.description
    }

    pub(crate) fn run_steps(&self) -> &[Step] {
        &self.state.run_steps
    }

    pub(crate) fn pending_steps(&self) -> &[Step] {
        &self.state.pending_steps
    }

    pub(crate) fn expected_result(&self) -> &str {
        &self.state.expected_result
    }

    pub(crate) fn run(&mut self) {
        if self.worker_started {
            Logger::warning(format!(
                "plan {} run ignored: worker already running (task {})",
                self.signature(),
                &self.signature().task,
            ));
            return;
        }
        let worker_plan = self.clone();
        self.worker_started = true;
        drop(self.state.access.rt.spawn(async move {
            worker_plan.worker().await;
        }));
    }
}

// -- Private -- //

impl Plan {
    async fn worker(self) {
        let Some(mut plan_rx) = self.take_receiver() else {
            Logger::warning(format!(
                "plan {} worker stopping: event receiver unavailable (task {})",
                self.signature(),
                &self.signature().task,
            ));
            return;
        };
        while let Some(event) = plan_rx.recv().await {
            if let Err(error) = self.dispatch(event) {
                Logger::debug(format!(
                    "plan {} worker stopping: {error:?} (task {})",
                    self.signature(),
                    &self.signature().task,
                ));
                break;
            }
        }
    }

    fn take_receiver(&self) -> Option<tokio::mpsc::UnboundedReceiver<PlanEvent>> {
        self.state
            .plan_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
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
                    self.signature(),
                    draft.name,
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
        access: TaskAccess,
        signature: &PlanSignature,
        drafts: Vec<StepDraft>,
    ) -> Result<Vec<Step>, PlanError> {
        drafts
            .into_iter()
            .map(|draft| Step::from_draft(access.clone(), signature, draft))
            .collect()
    }

    fn dispatch_step(&self, signature: StepSignature, event: StepEvent) {
        let event_name = format!("{event:?}");
        let Some(step) = self.step(&signature) else {
            Logger::error(format!(
                "step {} event {event_name} not dispatched: step not found (task {})",
                &signature, &signature.task,
            ));
            return;
        };
        if step.sender().send(event).is_err() {
            Logger::warning(format!(
                "step {} event {event_name} dispatch failed: worker stopped (task {})",
                &signature, &signature.task,
            ));
        }
    }

    fn cancel_steps(&self) {
        for step in &self.state.run_steps {
            self.send_task_event(TaskEvent::Plan(
                self.signature().clone(),
                PlanEvent::Step(step.signature().clone(), StepEvent::Cancel),
            ));
        }
    }

    fn send_task_event(&self, event: TaskEvent) {
        if self
            .state
            .access
            .session_tx
            .send(marix_protocol::SessionEvent::Task(
                self.signature().task.clone(),
                event,
            ))
            .is_err()
        {
            Logger::warning(format!(
                "plan {} update failed: session worker stopped (task {})",
                self.signature(),
                &self.signature().task,
            ));
        }
    }
}

struct PlanState {
    access: TaskAccess,
    signature: PlanSignature,
    description: String,
    run_steps: Vec<Step>,
    pending_steps: Vec<Step>,
    expected_result: String,
    plan_tx: tokio::mpsc::UnboundedSender<PlanEvent>,
    plan_rx: StdMutex<Option<tokio::mpsc::UnboundedReceiver<PlanEvent>>>,
}

impl PlanState {
    fn new(
        access: TaskAccess,
        signature: PlanSignature,
        description: String,
        run_steps: Vec<Step>,
        pending_steps: Vec<Step>,
        expected_result: String,
    ) -> Self {
        let (plan_tx, plan_rx) = build_async_channel();
        Self {
            access,
            signature,
            description,
            run_steps,
            pending_steps,
            expected_result,
            plan_tx,
            plan_rx: StdMutex::new(Some(plan_rx)),
        }
    }
}

impl fmt::Debug for Plan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let run_steps = self
            .state
            .run_steps
            .iter()
            .map(|step| step.signature())
            .collect::<Vec<_>>();
        let pending_steps = self
            .state
            .pending_steps
            .iter()
            .map(|step| step.signature())
            .collect::<Vec<_>>();
        formatter
            .debug_struct("Plan")
            .field("signature", self.signature())
            .field("description", &self.state.description)
            .field("run_steps", &run_steps)
            .field("pending_steps", &pending_steps)
            .field("expected_result", &self.state.expected_result)
            .finish()
    }
}
