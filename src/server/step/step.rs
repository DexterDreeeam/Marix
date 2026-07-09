use std::fmt;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex as StdMutex};
use std::thread::{self, JoinHandle};

use marix_common::{Logger, Receiver, Sender, build_channel};
use marix_protocol::{
    InvocationEvent, InvocationRequest, InvocationSignature, InvocationStatus, InvocationStepKind,
    ModelStepKind, PlanDraft, PlanError, PlanEvent, PlanSignature, RelayStatus, SessionEvent,
    StepDraft, StepError, StepEvent, StepKind, StepSignature, StepStatus, TaskEvent, TaskStatus,
    ToolInputSchema,
};

use crate::task::TaskState;

#[derive(Clone)]
pub struct Step {
    pub state: Arc<TaskState>,
    pub signature: StepSignature,
    pub description: String,
    pub kind: StepKind,
    pub update_count: Arc<AtomicUsize>,
    step_tx: Sender<StepEvent>,
    _worker: Arc<StdMutex<Option<JoinHandle<()>>>>,
}

impl Step {
    pub fn new(
        state: Arc<TaskState>,
        signature: StepSignature,
        description: String,
        kind: StepKind,
    ) -> Self {
        let (step_tx, step_rx) = build_channel();
        let worker = Arc::new(StdMutex::new(None));
        let step = Self {
            state,
            signature,
            description,
            kind,
            update_count: Arc::new(AtomicUsize::new(0)),
            step_tx,
            _worker: Arc::clone(&worker),
        };
        let worker_step = step.clone();
        let handle = thread::spawn(move || worker_step.worker(step_rx));
        *worker.lock().unwrap_or_else(|error| error.into_inner()) = Some(handle);
        step
    }

    pub(crate) fn from_draft(
        state: &Arc<TaskState>,
        plan: &PlanSignature,
        draft: StepDraft,
    ) -> Result<Self, PlanError> {
        let signature =
            StepSignature::new(state.signature.clone(), plan.clone(), draft.name.clone());
        let kind = Self::step_kind(&signature, &draft)?;
        Ok(Self::new(
            Arc::clone(state),
            signature,
            draft.description,
            kind,
        ))
    }

    pub(crate) fn sender(&self) -> Sender<StepEvent> {
        self.step_tx.clone()
    }

    pub(crate) fn start(&self) {
        if self.state.steps.with(&self.signature.id, |_| ()).is_some() {
            Logger::warning(format!(
                "step {} start ignored: step already exists (task {})",
                self.signature.id.0, self.signature.task.id.0
            ));
            return;
        }
        if self.state.steps.size() >= 10 {
            let reason = "task step limit exceeded".to_owned();
            Logger::warning(format!(
                "step {} rejected: {reason} (task {})",
                self.signature.id.0, self.signature.task.id.0
            ));
            self.fail_with_reason(reason);
            return;
        }
        self.state
            .steps
            .insert_or_update(self.signature.id.clone(), self.clone());
        match &self.kind {
            StepKind::Model(_) => self.clone().run_model(),
            StepKind::Invocation(InvocationStepKind::Invocation(request)) => {
                self.send_to_self(StepEvent::InvocationCreate(request.clone()));
            }
            StepKind::Invocation(InvocationStepKind::Cancel) => {
                self.fail_with_reason("invocation cancel step has no target".to_owned());
            }
            StepKind::Invocation(InvocationStepKind::Kill) => {
                self.fail_with_reason(
                    "invocation kill is not supported by the current protocol".to_owned(),
                );
            }
            StepKind::Intent | StepKind::User(_) => {
                self.fail_with_reason("task step kind is not supported yet".to_owned());
            }
        }
    }

    pub(crate) fn trigger_initial_plan(state: Arc<TaskState>) {
        let plan = PlanDraft {
            description: state.user_request.clone(),
            run_steps: vec![StepDraft {
                name: "Initial".to_owned(),
                kind: "model".to_owned(),
                description: state.user_request.clone(),
                input: "Initial".to_owned(),
            }],
            pending_steps: Vec::new(),
            expected_result: String::new(),
        };
        Self::send_task_event(&state, TaskEvent::PlanCreate(plan));
    }
}

// -- Private -- //

impl Step {
    fn worker(self, step_rx: Receiver<StepEvent>) {
        while let Ok(event) = step_rx.recv() {
            if let Err(error) = self.dispatch(event) {
                Logger::debug(format!(
                    "step {} worker stopping: {error:?} (task {})",
                    self.signature.id.0, self.signature.task.id.0
                ));
                break;
            }
        }
    }

    fn dispatch(&self, event: StepEvent) -> Result<(), StepError> {
        match event {
            StepEvent::Invocation(signature, event) => {
                self.dispatch_invocation(signature, event);
                Ok(())
            }
            StepEvent::InvocationCreate(request) => {
                self.create_invocation(request);
                Ok(())
            }
            StepEvent::InvocationUpdate(status) => {
                let error = Self::invocation_update_error(&status);
                self.on_invocation_update(status);
                if let Some(error) = error {
                    Err(error)
                } else {
                    Ok(())
                }
            }
            StepEvent::Relay(signature, event) => {
                self.dispatch_relay(signature, event);
                Ok(())
            }
            StepEvent::RelayCreate(request) => {
                self.create_relay(request);
                Ok(())
            }
            StepEvent::RelayUpdate(status) => {
                let error = Self::relay_update_error(&status);
                self.on_relay_update(status);
                if let Some(error) = error {
                    Err(error)
                } else {
                    Ok(())
                }
            }
            StepEvent::Cancel => {
                self.on_cancel();
                Err(StepError::Canceled)
            }
        }
    }

    fn invocation_update_error(status: &InvocationStatus) -> Option<StepError> {
        match status {
            InvocationStatus::Canceled => Some(StepError::InvocationCanceled),
            InvocationStatus::Succeed { .. } => Some(StepError::InvocationSucceeded),
            InvocationStatus::Failed => Some(StepError::InvocationFailed),
            InvocationStatus::Created
            | InvocationStatus::Started
            | InvocationStatus::Processing { .. } => None,
        }
    }

    fn relay_update_error(status: &RelayStatus) -> Option<StepError> {
        match status {
            RelayStatus::Canceled => Some(StepError::RelayCanceled),
            RelayStatus::Succeed { .. } => Some(StepError::RelaySucceeded),
            RelayStatus::Failed => Some(StepError::RelayFailed),
            RelayStatus::Created | RelayStatus::Started | RelayStatus::Processing { .. } => None,
        }
    }

    fn step_kind(signature: &StepSignature, draft: &StepDraft) -> Result<StepKind, PlanError> {
        match draft.kind.trim() {
            "tool" => Ok(StepKind::Invocation(InvocationStepKind::Invocation(
                InvocationRequest {
                    signature: InvocationSignature::new(
                        signature.task.clone(),
                        signature.plan.clone(),
                        signature.clone(),
                        draft.name.clone(),
                    ),
                    input: ToolInputSchema {
                        content: draft.input.clone(),
                    },
                },
            ))),
            "intent" => Ok(StepKind::Intent),
            "model" => Ok(StepKind::Model(Self::model_step_kind(draft)?)),
            kind => Err(PlanError::InvalidStepKind(kind.to_owned())),
        }
    }

    fn model_step_kind(draft: &StepDraft) -> Result<ModelStepKind, PlanError> {
        Self::parse_model_step_name(&draft.name)
            .or_else(|| Self::parse_model_step_name(Self::input_model_name(&draft.input)))
            .ok_or_else(|| PlanError::InvalidModelStep {
                name: draft.name.clone(),
                input: draft.input.clone(),
            })
    }

    fn parse_model_step_name(name: &str) -> Option<ModelStepKind> {
        match name.trim() {
            "Initial" | "initial" => Some(ModelStepKind::Initial),
            "Analysis" | "analysis" => Some(ModelStepKind::Analysis),
            _ => None,
        }
    }

    fn input_model_name(input: &str) -> &str {
        input.split(',').next().unwrap_or_default().trim()
    }

    pub(super) fn complete(&self, content: String) {
        if !self.complete_current() {
            return;
        }
        if self.state.plan_hub.complete_step(&self.signature).is_none() {
            return;
        }
        Self::send_plan_event(
            &self.state,
            self.signature.plan.clone(),
            PlanEvent::StepUpdate(StepStatus::Succeed),
        );
        match &self.kind {
            StepKind::Model(_) => self.on_model_complete(&content),
            StepKind::Invocation(_) => self.on_invocation_complete(&content),
            StepKind::Intent | StepKind::User(_) => {}
        }
    }

    pub(super) fn fail_with_reason(&self, reason: String) {
        Logger::error(format!(
            "step {} failed: {reason} (task {})",
            self.signature.id.0, self.signature.task.id.0
        ));
        self.complete_current();
        Self::send_plan_event(
            &self.state,
            self.signature.plan.clone(),
            PlanEvent::StepUpdate(StepStatus::Failed),
        );
    }

    fn complete_current(&self) -> bool {
        if !self.is_working() {
            Logger::warning(format!(
                "step {} completion ignored: step is not working (task {})",
                self.signature.id.0, self.signature.task.id.0
            ));
            return false;
        }
        self.state.steps.complete(self.signature.id.clone());
        true
    }

    fn is_working(&self) -> bool {
        self.state
            .steps
            .working_list()
            .iter()
            .any(|step| step.signature.id == self.signature.id)
    }

    fn on_cancel(&self) {
        match &self.kind {
            StepKind::Invocation(InvocationStepKind::Invocation(request)) => {
                self.dispatch_invocation(request.signature.clone(), InvocationEvent::Cancel);
            }
            StepKind::Model(_) => {
                Logger::warning(format!(
                    "model step {} cancel requested, but model cancellation is not supported",
                    self.signature.id.0
                ));
                self.fail_with_reason("model cancellation is not supported".to_owned());
            }
            StepKind::Intent | StepKind::User(_) | StepKind::Invocation(_) => {
                self.fail_with_reason("step canceled".to_owned());
            }
        }
    }

    fn send_to_self(&self, event: StepEvent) {
        if self.sender().send(event).is_err() {
            Logger::warning(format!(
                "step {} self event failed: worker stopped (task {})",
                self.signature.id.0, self.signature.task.id.0
            ));
        }
    }

    pub(super) fn send_task_update(state: &TaskState, status: TaskStatus) {
        if state
            .task_tx
            .send(SessionEvent::TaskUpdate(status))
            .is_err()
        {
            Logger::warning(format!(
                "task {} update failed: task worker stopped",
                state.signature.id.0
            ));
        }
    }

    pub(super) fn send_task_event(state: &TaskState, event: TaskEvent) {
        if state
            .task_tx
            .send(SessionEvent::Task(state.signature.clone(), event))
            .is_err()
        {
            Logger::warning(format!(
                "task {} event failed: task worker stopped",
                state.signature.id.0
            ));
        }
    }

    fn send_plan_event(state: &TaskState, signature: PlanSignature, event: PlanEvent) {
        Self::send_task_event(state, TaskEvent::Plan(signature, event));
    }
}

impl fmt::Debug for Step {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Step")
            .field("signature", &self.signature)
            .field("description", &self.description)
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}
