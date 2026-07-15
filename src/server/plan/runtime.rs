use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::external::*;
use marix_common::{AsyncReceiver, AsyncSender, Logger, build_async_channel};
use marix_protocol::{
    IntentResultKind, IntentSignature, IntentStatus, PlanDraft,
    PlanEvent, PlanResult, PlanResultKind, PlanStatus, PlanVerdict,
    RelayRequest, RelaySignature, RelayStatus, SessionEvent, TaskEvent,
};

use super::PlanState;
use crate::intent::Intent;
use crate::relay::Relay;

pub struct PlanRuntime {
    pub(super) state: Arc<PlanState>,
    pub(super) close_tx: AsyncSender<()>,
    close_rx: StdMutex<Option<AsyncReceiver<()>>>,
}

impl PlanRuntime {
    pub async fn run(&self) {
        let Some(mut plan_rx) = self
            .state
            .plan_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
        else {
            Logger::warning(format!(
                "plan {} start ignored: already running",
                &self.state.signature,
            ));
            return;
        };
        self.set_status(PlanStatus::Running);
        self.advance();
        let Some(mut close_rx) = self
            .close_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
        else {
            self.finish(
                PlanResultKind::Failed,
                "plan close receiver unavailable".to_owned(),
            );
            return;
        };

        loop {
            self::tokio::select! {
                _ = close_rx.recv() => break,
                event = plan_rx.recv() => {
                    let Some(event) = event else {
                        break;
                    };
                    self.dispatch(event);
                }
            }
        }
    }

    pub fn dispatch(&self, event: PlanEvent) {
        match event {
            PlanEvent::Update(signature, status) => {
                self.on_intent_update(signature, status);
            }
            PlanEvent::RelayUpdate(signature, status) => {
                self.on_relay_update(signature, status);
            }
            PlanEvent::Cancel => {
                self.cancel("plan canceled".to_owned());
            }
        }
    }
}

// -- Private -- //

impl PlanRuntime {
    pub(crate) fn new(state: Arc<PlanState>) -> Self {
        let (close_tx, close_rx) = build_async_channel();
        Self {
            state,
            close_tx,
            close_rx: StdMutex::new(Some(close_rx)),
        }
    }

    fn advance(&self) {
        let intents = self
            .state
            .intents
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        let mut latest_output = String::new();
        for signature in intents {
            let Some(result) = self.state.access.get_result(&signature) else {
                if let Err(reason) = self.start_intent(&signature) {
                    self.finish(PlanResultKind::Failed, reason);
                }
                return;
            };
            match result.kind {
                IntentResultKind::Succeed => {
                    latest_output = result.output;
                }
                IntentResultKind::Infeasible
                | IntentResultKind::Canceled
                | IntentResultKind::Failed => return,
            }
        }
        self.finish(PlanResultKind::Succeed, latest_output);
    }

    fn on_intent_update(
        &self,
        signature: IntentSignature,
        status: IntentStatus,
    ) {
        if self.status().is_terminal() {
            Logger::error(format!(
                "plan {} received child intent {signature} update \
                 {status:?} after completion",
                &self.state.signature,
            ));
            return;
        }
        let IntentStatus::Complete(result) = status else {
            return;
        };
        let contains_intent = self
            .state
            .intents
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .contains(&signature);
        if !contains_intent {
            self.finish(
                PlanResultKind::Failed,
                format!("plan child intent {signature} not found"),
            );
            return;
        }
        match result.kind {
            IntentResultKind::Succeed => self.advance(),
            IntentResultKind::Infeasible => {
                self.verdict(
                    PlanResultKind::Infeasible,
                    result.output,
                );
            }
            IntentResultKind::Canceled => self.cancel(result.output),
            IntentResultKind::Failed => {
                self.verdict(PlanResultKind::Failed, result.output);
            }
        }
    }

    fn start_intent(
        &self,
        signature: &IntentSignature,
    ) -> Result<(), String> {
        let event = SessionEvent::Task(
            self.state.access.signature.clone(),
            TaskEvent::IntentStart(signature.clone()),
        );
        self.state.access.session_tx.send(event).map_err(|_| {
            format!(
                "plan child intent {signature} start failed: session stopped"
            )
        })
    }

    fn verdict(&self, kind: PlanResultKind, output: String) {
        let request = RelayRequest {
            signature: RelaySignature::new(
                self.state.signature.intent.as_ref().clone(),
                "plan-verdict".to_owned(),
            ),
            prompt: format!("{kind:?}: {output}"),
        };
        let relay = match Relay::new(
            Arc::clone(&self.state.access),
            request,
        ) {
            Ok(relay) => relay,
            Err(reason) => {
                self.finish(PlanResultKind::Failed, reason);
                return;
            }
        };
        if !self.state.access.insert_relay(relay.clone()) {
            self.finish(
                PlanResultKind::Failed,
                format!(
                    "plan verdict relay {} already exists",
                    &relay.state.signature,
                ),
            );
            return;
        }
        relay.start();
    }

    fn on_relay_update(
        &self,
        signature: RelaySignature,
        status: RelayStatus,
    ) {
        if self.status().is_terminal() {
            Logger::error(format!(
                "plan {} received relay {signature} update {status:?} \
                 after completion",
                &self.state.signature,
            ));
            return;
        }
        if !status.is_terminal() {
            return;
        }
        match status {
            RelayStatus::Succeed { .. } => {
                let Some(output) = self
                    .state
                    .access
                    .get_relay_result(&signature)
                else {
                    self.finish(
                        PlanResultKind::Failed,
                        format!(
                            "plan relay {signature} succeeded without output"
                        ),
                    );
                    return;
                };
                match PlanVerdict::parse(&output) {
                    Ok(PlanVerdict::Replacement(draft)) => {
                        if let Err(reason) = self.reconstruct(draft) {
                            self.finish(PlanResultKind::Failed, reason);
                        }
                    }
                    Ok(PlanVerdict::Infeasible { reason }) => {
                        self.finish(PlanResultKind::Infeasible, reason);
                    }
                    Err(error) => {
                        self.finish(
                            PlanResultKind::Failed,
                            format!(
                                "plan verdict from relay {signature} is \
                                 malformed: {error}"
                            ),
                        );
                    }
                }
            }
            RelayStatus::Failed => {
                self.finish(
                    PlanResultKind::Failed,
                    format!("plan relay {signature} failed"),
                );
            }
            RelayStatus::Canceled => {
                self.cancel("plan reconstruction canceled".to_owned());
            }
            RelayStatus::Created | RelayStatus::Started => {}
        }
    }

    fn reconstruct(&self, draft: PlanDraft) -> Result<(), String> {
        let current_intents = self
            .state
            .intents
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        let mut failure = None;
        for signature in &current_intents {
            let Some(result) = self.state.access.get_result(signature) else {
                continue;
            };
            let kind = match result.kind {
                IntentResultKind::Succeed => continue,
                IntentResultKind::Infeasible => {
                    PlanResultKind::Infeasible
                }
                IntentResultKind::Failed => PlanResultKind::Failed,
                IntentResultKind::Canceled => PlanResultKind::Canceled,
            };
            failure = Some(PlanResult {
                kind,
                output: result.output,
            });
            break;
        }
        let failure = failure.ok_or_else(|| {
            "plan reconstruction has no failed intent result".to_owned()
        })?;
        let failure_count = {
            let mut failures = self
                .state
                .failures
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            failures.push(failure);
            failures.len()
        };
        if draft.intents.is_empty() {
            return Err(
                "plan reconstruction must contain a child intent".to_owned()
            );
        }
        for (index, intent) in draft.intents.iter().enumerate() {
            if intent.content.trim().is_empty() {
                return Err(format!(
                    "plan reconstruction child intent {} has empty content",
                    index + 1,
                ));
            }
        }

        let mut signatures = Vec::with_capacity(draft.intents.len());
        for (index, draft) in draft.intents.into_iter().enumerate() {
            let signature = IntentSignature::new(
                self.state.access.signature.clone(),
                Some(self.state.signature.clone()),
                format!("intent-r{failure_count}-{}", index + 1),
            );
            let intent = Intent::new(
                Arc::clone(&self.state.access),
                signature.clone(),
                draft.content,
            );
            if !self.state.access.insert_intent(intent) {
                return Err(format!(
                    "plan reconstruction child intent {signature} is duplicated"
                ));
            }
            signatures.push(signature);
        }
        *self
            .state
            .intents
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = signatures;
        self.advance();
        Ok(())
    }
}
