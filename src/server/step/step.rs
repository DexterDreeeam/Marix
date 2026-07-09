use std::fmt;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::external::*;
use marix_common::{Logger, build_async_channel};
use marix_protocol::{
    InvocationEvent, InvocationRequest, InvocationSignature, InvocationStatus, InvocationStepKind,
    ModelStepKind, PlanDraft, PlanError, PlanEvent, PlanSignature, RelayStatus, SessionEvent,
    StepDraft, StepError, StepEvent, StepKind, StepSignature, StepStatus, TaskEvent, TaskStatus,
    ToolInputSchema,
};

use crate::task::TaskAccess;

pub struct Step {
    pub(super) state: Arc<StepState>,
    worker_started: bool,
}

impl Clone for Step {
    fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
            worker_started: false,
        }
    }
}

impl Step {
    pub fn new(
        access: TaskAccess,
        signature: StepSignature,
        description: String,
        kind: StepKind,
    ) -> Self {
        let step = Self {
            state: Arc::new(StepState::new(signature, description, kind, access)),
            worker_started: false,
        };
        Self::send_plan_event(
            &step.state,
            step.state.signature.plan.clone(),
            PlanEvent::StepUpdate(StepStatus::Created),
        );
        step
    }

    pub(crate) fn from_draft(
        access: TaskAccess,
        plan: &PlanSignature,
        draft: StepDraft,
    ) -> Result<Self, PlanError> {
        let signature =
            StepSignature::new(access.signature.clone(), plan.clone(), draft.name.clone());
        let kind = Self::step_kind(&signature, &draft)?;
        Ok(Self::new(access, signature, draft.description, kind))
    }

    pub(crate) fn sender(&self) -> tokio::mpsc::UnboundedSender<StepEvent> {
        self.state.step_tx.clone()
    }

    pub(crate) fn signature(&self) -> &StepSignature {
        &self.state.signature
    }

    pub(crate) fn description(&self) -> &str {
        &self.state.description
    }

    pub(crate) fn kind(&self) -> &StepKind {
        &self.state.kind
    }

    pub(crate) fn run(&mut self) {
        if self.worker_started {
            Logger::warning(format!(
                "step {} run ignored: worker already running (task {})",
                &self.state.signature, &self.state.signature.task,
            ));
            return;
        }
        let worker_step = self.clone();
        self.worker_started = true;
        drop(self.state.access.rt.spawn(async move {
            worker_step.worker().await;
        }));
    }

    pub(crate) fn trigger_initial_plan(access: TaskAccess) {
        let user_request = access.user_request.clone();
        let plan = PlanDraft {
            description: user_request.clone(),
            run_steps: vec![StepDraft {
                name: "Initial".to_owned(),
                kind: "model".to_owned(),
                description: user_request,
                input: "Initial".to_owned(),
            }],
            pending_steps: Vec::new(),
            expected_result: String::new(),
        };
        if access
            .session_tx
            .send(SessionEvent::Task(
                access.signature.clone(),
                TaskEvent::PlanCreate(plan),
            ))
            .is_err()
        {
            Logger::warning(format!(
                "task {} event failed: session worker stopped",
                &access.signature,
            ));
        }
    }

    pub(crate) fn complete(&self, content: String) {
        Self::send_plan_event(
            &self.state,
            self.state.signature.plan.clone(),
            PlanEvent::StepUpdate(StepStatus::Succeed),
        );
        match &self.state.kind {
            StepKind::Model(_) => self.on_model_complete(&content),
            StepKind::Invocation(_) => self.on_invocation_complete(&content),
            StepKind::Intent | StepKind::User(_) => {}
        }
    }

    pub(crate) fn fail_with_reason(&self, reason: String) {
        Logger::error(format!(
            "step {} failed: {reason} (task {})",
            &self.state.signature, &self.state.signature.task,
        ));
        Self::send_plan_event(
            &self.state,
            self.state.signature.plan.clone(),
            PlanEvent::StepUpdate(StepStatus::Failed),
        );
    }
}

// -- Private -- //

impl Step {
    async fn worker(self) {
        Self::send_plan_event(
            &self.state,
            self.state.signature.plan.clone(),
            PlanEvent::StepUpdate(StepStatus::Started),
        );
        let Some(mut step_rx) = self.take_receiver() else {
            Logger::warning(format!(
                "step {} worker stopping: event receiver unavailable (task {})",
                &self.state.signature, &self.state.signature.task,
            ));
            return;
        };
        while let Some(event) = step_rx.recv().await {
            if let Err(error) = self.dispatch(event) {
                Logger::debug(format!(
                    "step {} worker stopping: {error:?} (task {})",
                    &self.state.signature, &self.state.signature.task,
                ));
                break;
            }
        }
    }

    fn take_receiver(&self) -> Option<tokio::mpsc::UnboundedReceiver<StepEvent>> {
        self.state
            .step_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
    }

    fn dispatch(&self, event: StepEvent) -> Result<(), StepError> {
        match event {
            StepEvent::Invocation(signature, event) => {
                Logger::warning(format!(
                    "step {} received invocation event for {} after runtime routing",
                    &self.state.signature, &signature,
                ));
                let _ = event;
                Ok(())
            }
            StepEvent::InvocationCreate(request) => {
                Logger::warning(format!(
                    "step {} received invocation create for {} after runtime routing",
                    &self.state.signature, &request.signature,
                ));
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
                Logger::warning(format!(
                    "step {} received relay event for {} after runtime routing",
                    &self.state.signature, &signature,
                ));
                let _ = event;
                Ok(())
            }
            StepEvent::RelayCreate(request) => {
                Logger::warning(format!(
                    "step {} received relay create for {} after runtime routing",
                    &self.state.signature, &request.signature,
                ));
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

    fn on_cancel(&self) {
        match &self.state.kind {
            StepKind::Invocation(InvocationStepKind::Invocation(request)) => {
                Self::send_plan_event(
                    &self.state,
                    self.state.signature.plan.clone(),
                    PlanEvent::Step(
                        self.state.signature.clone(),
                        StepEvent::Invocation(request.signature.clone(), InvocationEvent::Cancel),
                    ),
                );
            }
            StepKind::Model(_) => {
                Logger::warning(format!(
                    "model step {} cancel requested, but model cancellation is not supported",
                    &self.state.signature,
                ));
                self.fail_with_reason("model cancellation is not supported".to_owned());
            }
            StepKind::Intent | StepKind::User(_) | StepKind::Invocation(_) => {
                self.fail_with_reason("step canceled".to_owned());
            }
        }
    }

    pub(super) fn send_task_update(state: &StepState, status: TaskStatus) {
        if state
            .access
            .session_tx
            .send(SessionEvent::TaskUpdate(status))
            .is_err()
        {
            Logger::warning(format!(
                "task {} update failed: session worker stopped",
                &state.access.signature,
            ));
        }
    }

    pub(super) fn send_task_event(state: &StepState, event: TaskEvent) {
        if state
            .access
            .session_tx
            .send(SessionEvent::Task(state.access.signature.clone(), event))
            .is_err()
        {
            Logger::warning(format!(
                "task {} event failed: session worker stopped",
                &state.access.signature,
            ));
        }
    }

    fn send_plan_event(state: &StepState, signature: PlanSignature, event: PlanEvent) {
        Self::send_task_event(state, TaskEvent::Plan(signature, event));
    }
}

pub(super) struct StepState {
    pub(super) signature: StepSignature,
    pub(super) description: String,
    pub(super) kind: StepKind,
    pub(super) access: TaskAccess,
    pub(super) step_tx: tokio::mpsc::UnboundedSender<StepEvent>,
    pub(super) step_rx: StdMutex<Option<tokio::mpsc::UnboundedReceiver<StepEvent>>>,
}

impl StepState {
    fn new(
        signature: StepSignature,
        description: String,
        kind: StepKind,
        access: TaskAccess,
    ) -> Self {
        let (step_tx, step_rx) = build_async_channel();
        Self {
            signature,
            description,
            kind,
            access,
            step_tx,
            step_rx: StdMutex::new(Some(step_rx)),
        }
    }
}

impl fmt::Debug for Step {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Step")
            .field("signature", &self.state.signature)
            .field("description", &self.state.description)
            .field("kind", &self.state.kind)
            .finish_non_exhaustive()
    }
}
