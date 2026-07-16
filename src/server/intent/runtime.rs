use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::external::*;
use marix_common::{AsyncReceiver, AsyncSender, Logger, build_async_channel};
use marix_protocol::{
    IntentEvent, IntentResult, IntentResultKind, IntentStatus,
    IntentVerdict, PlanEvent, PlanResultKind, PlanSignature, PlanStatus,
    RelayRequest, RelayResultKind, RelaySignature, RelayStatus, SessionEvent,
    StepEvent, StepResultKind, StepSignature, StepStatus,
    TaskEvent,
};

use super::IntentState;
use crate::relay::Relay;

pub struct IntentRuntime {
    pub state: Arc<IntentState>,
    pub close_tx: AsyncSender<()>,
    pub close_rx: StdMutex<Option<AsyncReceiver<()>>>,
}

impl IntentRuntime {
    pub async fn run(&self) {
        let Some(mut intent_rx) = self
            .state
            .intent_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
        else {
            Logger::warning(format!(
                "intent {} start ignored: already running",
                &self.state.signature,
            ));
            return;
        };
        self.set_status(IntentStatus::Running);
        Logger::log(format!("intent {} started", &self.state.signature,));
        if let Err(reason) = self.verdict() {
            self.fail(reason);
            return;
        }

        let Some(mut close_rx) = self
            .close_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
        else {
            self.fail("intent close receiver unavailable".to_owned());
            return;
        };

        loop {
            self::tokio::select! {
                _ = close_rx.recv() => break,
                event = intent_rx.recv() => {
                    let Some(event) = event else {
                        break;
                    };
                    self.dispatch(event);
                }
            }
        }
    }

    pub fn dispatch(&self, event: IntentEvent) {
        match event {
            IntentEvent::PlanUpdate(signature, status) => {
                self.on_plan_update(signature, status);
            }
            IntentEvent::StepUpdate(signature, status) => {
                self.on_step_update(signature, status);
            }
            IntentEvent::RelayUpdate(signature, status) => {
                self.on_relay_update(signature, status);
            }
            IntentEvent::Cancel => self.cancel(),
        }
    }
}

// -- Private -- //

impl IntentRuntime {
    pub(crate) fn new(state: Arc<IntentState>) -> Self {
        let (close_tx, close_rx) = build_async_channel();
        Self {
            state,
            close_tx,
            close_rx: StdMutex::new(Some(close_rx)),
        }
    }

    fn verdict(&self) -> Result<(), String> {
        let request = RelayRequest {
            signature: RelaySignature::new(
                self.state.signature.clone(),
                "intent-verdict".to_owned(),
            ),
            prompt: self.state.content.clone(),
        };
        let relay = Relay::new(
            Arc::clone(&self.state.access),
            request,
        )?;
        if !self.state.access.insert_relay(relay.clone()) {
            return Err(format!(
                "intent verdict relay {} already exists",
                &relay.state.signature,
            ));
        }
        relay.start();
        Ok(())
    }

    fn on_plan_update(
        &self,
        signature: PlanSignature,
        status: PlanStatus,
    ) {
        if self.status().is_terminal() {
            Logger::error(format!(
                "intent {} received plan {signature} update {status:?} \
                 after completion",
                &self.state.signature,
            ));
            return;
        }
        let PlanStatus::Complete(result) = status else {
            return;
        };
        let has_plan = self
            .state
            .plan
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .is_some();
        if !has_plan {
            self.fail(format!(
                "intent received plan update for {signature} without a plan"
            ));
            return;
        }
        match result.kind {
            PlanResultKind::Succeed => {
                self.finish(IntentResultKind::Succeed, result.output);
            }
            PlanResultKind::Infeasible => {
                self.finish(IntentResultKind::Infeasible, result.output);
            }
            PlanResultKind::Canceled => {
                self.finish(IntentResultKind::Canceled, result.output);
            }
            PlanResultKind::Failed => self.fail(result.output),
        }
    }

    fn on_relay_update(&self, signature: RelaySignature, status: RelayStatus) {
        if self.status().is_terminal() {
            Logger::error(format!(
                "intent {} received relay {signature} update {status:?} \
                 after completion",
                &self.state.signature,
            ));
            return;
        }
        let RelayStatus::Complete(result) = status else {
            return;
        };
        let plan = self
            .state
            .plan
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        if let Some(plan) = plan {
            self.send_plan_event(
                plan,
                PlanEvent::RelayUpdate(
                    signature,
                    RelayStatus::Complete(result),
                ),
            );
            return;
        }
        match result.kind {
            RelayResultKind::Succeed => {
                let verdict = match IntentVerdict::parse(&result.output) {
                    Ok(verdict) => verdict,
                    Err(error) => {
                        self.fail(format!(
                            "intent verdict from relay {signature} is \
                             malformed: {error}"
                        ));
                        return;
                    }
                };
                self.on_verdict(verdict);
            }
            RelayResultKind::Failed => {
                self.finish(IntentResultKind::Failed, result.output);
            }
            RelayResultKind::Canceled => {
                self.finish(IntentResultKind::Canceled, result.output);
            }
        }
    }

    fn on_step_update(&self, signature: StepSignature, status: StepStatus) {
        if self.status().is_terminal() {
            Logger::error(format!(
                "intent {} received step {signature} update {status:?} \
                 after completion",
                &self.state.signature,
            ));
            return;
        }
        let StepStatus::Complete(result) = status else {
            return;
        };
        let Some(updated) =
            self.state.steps.with_mut(&signature, |stored| {
                if stored.is_some() {
                    return false;
                }
                *stored = Some(result.clone());
                true
            })
        else {
            self.fail(format!("step {signature} not found"));
            return;
        };
        if !updated {
            Logger::error(format!(
                "intent {} received duplicate complete update from step \
                 {signature}",
                &self.state.signature,
            ));
            return;
        }
        match result.kind {
            StepResultKind::Succeed => {
                if let Err(reason) = self.verdict() {
                    self.fail(reason);
                }
            }
            StepResultKind::Failed => {
                self.finish(IntentResultKind::Failed, result.output);
            }
            StepResultKind::Canceled => {
                self.finish(IntentResultKind::Canceled, result.output);
            }
        }
    }

    fn on_verdict(&self, verdict: IntentVerdict) {
        match verdict {
            IntentVerdict::Step(draft) => {
                if let Err(reason) = self.create_step(draft) {
                    self.fail(reason);
                }
            }
            IntentVerdict::Plan(draft) => {
                if let Err(reason) = self.create_plan(draft) {
                    self.fail(reason);
                }
            }
            IntentVerdict::Complete { output } => {
                self.finish(IntentResultKind::Succeed, output);
            }
            IntentVerdict::Infeasible { reason } => {
                self.finish(IntentResultKind::Infeasible, reason);
            }
        }
    }

    pub(super) fn cancel(&self) {
        if self.status().is_terminal() {
            return;
        }
        let plan = self
            .state
            .plan
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        if let Some(plan) = plan {
            self.send_plan_event(plan, PlanEvent::Cancel);
        }
        for (signature, result) in self.state.steps.entries() {
            if result.is_some() {
                continue;
            }
            let event = SessionEvent::Task(
                self.state.access.signature.clone(),
                TaskEvent::Step(
                    signature.clone(),
                    StepEvent::Cancel,
                ),
            );
            if self.state.access.session_tx.send(event).is_err() {
                Logger::warning(format!(
                    "intent {} step {signature} cancel failed: session \
                     stopped",
                    &self.state.signature,
                ));
            }
        }
        self.finish(
            IntentResultKind::Canceled,
            "intent canceled".to_owned(),
        );
    }

    pub(super) fn fail(&self, reason: String) {
        Logger::error(format!("intent {} failed: {reason}", &self.state.signature,));
        self.finish(IntentResultKind::Failed, reason);
    }

    pub(super) fn finish(
        &self,
        kind: IntentResultKind,
        output: String,
    ) {
        let result = IntentResult { kind, output };
        let status = IntentStatus::Complete(result);
        self.set_status(status);
        self.close();
    }

    fn status(&self) -> IntentStatus {
        self.state
            .status
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    pub(super) fn set_status(&self, status: IntentStatus) {
        let mut current = self
            .state
            .status
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if current.is_terminal() {
            return;
        }
        let send_update = matches!(&status, IntentStatus::Complete(_));
        *current = status.clone();
        drop(current);
        if send_update {
            self.send_task_update(status);
        }
    }

    fn send_task_update(&self, status: IntentStatus) {
        let event = match self.state.signature.parent.clone() {
            None => TaskEvent::Update(
                self.state.signature.clone(),
                status,
            ),
            Some(parent) => TaskEvent::Plan(
                parent,
                PlanEvent::Update(self.state.signature.clone(), status),
            ),
        };
        let task_event = SessionEvent::Task(
            self.state.access.signature.clone(),
            event,
        );
        if self.state.access.session_tx.send(task_event).is_err() {
            Logger::warning(format!(
                "intent {} event send failed: session stopped",
                &self.state.signature,
            ));
        }
    }

    fn send_plan_event(
        &self,
        signature: PlanSignature,
        event: PlanEvent,
    ) {
        let task_event = SessionEvent::Task(
            self.state.access.signature.clone(),
            TaskEvent::Plan(signature, event),
        );
        if self.state.access.session_tx.send(task_event).is_err() {
            Logger::warning(format!(
                "intent {} plan event send failed: session stopped",
                &self.state.signature,
            ));
        }
    }

    fn close(&self) {
        if self.close_tx.send(()).is_err() {
            Logger::warning(format!(
                "intent {} close signal failed",
                &self.state.signature,
            ));
        }
    }
}
