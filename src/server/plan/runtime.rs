use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::{
    Actor, ActorStartFuture, ActorStatus, Lifecycle, Logger, Runtime as RuntimeTrait,
};
use marix_protocol::{
    IntentEvent, IntentResult, IntentResultKind, IntentSignature, PlanDraft, PlanEvent, PlanResult,
    PlanResultKind, PlanSignature, PlanVerdict, RelayRequest, RelayResult, RelayResultKind,
    RelaySignature, SessionEvent, TaskEvent,
};

use super::Plan;
use crate::intent::Intent;
use crate::prompt::{MessagePrompt, Prompt};
use crate::relay::Relay;
use crate::task::TaskAccess;

pub struct PlanRuntime {
    pub access: Arc<TaskAccess>,
    pub signature: PlanSignature,
    pub intents: StdMutex<Vec<IntentSignature>>,
    pub failures: StdMutex<Vec<PlanResult>>,
    pub lifecycle: Lifecycle<PlanEvent, PlanResult>,
}

impl PlanRuntime {
    pub(crate) fn new(
        access: Arc<TaskAccess>,
        signature: PlanSignature,
        intents: Vec<IntentSignature>,
    ) -> Self {
        Self {
            access,
            signature,
            intents: StdMutex::new(intents),
            failures: StdMutex::new(Vec::new()),
            lifecycle: Lifecycle::new(),
        }
    }
}

impl RuntimeTrait for PlanRuntime {
    type Base = Plan;
    type Prepared = ();

    fn signature(&self) -> &PlanSignature {
        &self.signature
    }

    fn lifecycle(&self) -> &Lifecycle<PlanEvent, PlanResult> {
        &self.lifecycle
    }

    fn on_start(&self) -> ActorStartFuture<'_, Self::Prepared> {
        Box::pin(async move {
            self.advance();
            Some(())
        })
    }

    fn dispatch(&self, event: PlanEvent) {
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

    fn on_finish(&self, result: PlanResult) {
        self.send_parent_event(IntentEvent::PlanUpdate(
            self.signature.clone(),
            ActorStatus::Complete(result),
        ));
    }
}

// -- Private -- //

impl PlanRuntime {
    fn advance(&self) {
        let intents = self
            .intents
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        let mut latest_output = String::new();
        for signature in intents {
            let Some(result) = self.access.get_result(&signature) else {
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

    fn on_intent_update(&self, signature: IntentSignature, status: ActorStatus<IntentResult>) {
        if matches!(self.status(), ActorStatus::Complete(_)) {
            Logger::error(format!(
                "plan {} received child intent {signature} update \
                 {status:?} after completion",
                &self.signature,
            ));
            return;
        }
        let ActorStatus::Complete(result) = status else {
            return;
        };
        let contains_intent = self
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
                self.verdict(PlanResultKind::Infeasible, result.output);
            }
            IntentResultKind::Canceled => {
                self.cancel(result.output);
            }
            IntentResultKind::Failed => {
                self.verdict(PlanResultKind::Failed, result.output);
            }
        }
    }

    fn start_intent(&self, signature: &IntentSignature) -> Result<(), String> {
        let event = SessionEvent::Task(
            self.access.signature.clone(),
            TaskEvent::IntentStart(signature.clone()),
        );
        self.access.session_tx.send(event).map_err(|_| {
            format!(
                "plan child intent {signature} start failed: \
                 session stopped",
            )
        })
    }

    fn verdict(&self, kind: PlanResultKind, output: String) {
        self.failures
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .push(PlanResult { kind, output });
        let prompt = match self.verdict_prompt() {
            Ok(prompt) => prompt,
            Err(reason) => {
                self.finish(PlanResultKind::Failed, reason);
                return;
            }
        };
        let request = RelayRequest {
            signature: RelaySignature::new(
                self.signature.intent.as_ref().clone(),
                Some(self.signature.clone()),
                "plan-verdict".to_owned(),
            ),
            prompt,
        };
        let relay = match Relay::new(Arc::clone(&self.access), request) {
            Ok(relay) => relay,
            Err(reason) => {
                self.finish(PlanResultKind::Failed, reason);
                return;
            }
        };
        if !self.access.insert(relay.clone()) {
            self.finish(
                PlanResultKind::Failed,
                format!("plan verdict relay {} already exists", relay.signature(),),
            );
            return;
        }
        relay.start();
    }

    fn verdict_prompt(&self) -> Result<String, String> {
        let message_prompt = MessagePrompt::PlanVerdict;
        let prompt = std::panic::catch_unwind(|| Prompt::load(message_prompt.name())).map_err(
            |payload| {
                let detail = if let Some(message) = payload.downcast_ref::<String>() {
                    message.clone()
                } else if let Some(message) = payload.downcast_ref::<&str>() {
                    (*message).to_owned()
                } else {
                    "unknown prompt loading panic".to_owned()
                };
                format!("failed to load PlanVerdict prompt: {detail}",)
            },
        )?;
        prompt
            .prompt()
            .map_err(|error| format!("failed to render PlanVerdict prompt: {error}"))
    }

    fn on_relay_update(&self, signature: RelaySignature, status: ActorStatus<RelayResult>) {
        if matches!(self.status(), ActorStatus::Complete(_)) {
            Logger::error(format!(
                "plan {} received relay {signature} update \
                 {status:?} after completion",
                &self.signature,
            ));
            return;
        }
        let ActorStatus::Complete(result) = status else {
            return;
        };
        match result.kind {
            RelayResultKind::Succeed => match PlanVerdict::parse(&result.output) {
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
                            "plan verdict from relay {signature} \
                                 is malformed: {error}",
                        ),
                    );
                }
            },
            RelayResultKind::Failed => {
                self.finish(PlanResultKind::Failed, result.output);
            }
            RelayResultKind::Canceled => {
                self.cancel(result.output);
            }
        }
    }

    fn reconstruct(&self, draft: PlanDraft) -> Result<(), String> {
        let failure_count = self
            .failures
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .len();
        if draft.intents.is_empty() {
            return Err("plan reconstruction must contain a child intent".to_owned());
        }
        for (index, intent) in draft.intents.iter().enumerate() {
            if intent.content.trim().is_empty() {
                return Err(format!(
                    "plan reconstruction child intent {} has empty \
                     content",
                    index + 1,
                ));
            }
        }

        let mut signatures = Vec::with_capacity(draft.intents.len());
        for (index, draft) in draft.intents.into_iter().enumerate() {
            let signature = IntentSignature::new(
                self.access.signature.clone(),
                Some(self.signature.clone()),
                format!("intent-r{failure_count}-{}", index + 1,),
            );
            let intent = Intent::new(Arc::clone(&self.access), signature.clone(), draft.content);
            if !self.access.insert(intent) {
                return Err(format!(
                    "plan reconstruction child intent {signature} \
                     is duplicated",
                ));
            }
            signatures.push(signature);
        }
        *self
            .intents
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = signatures;
        self.advance();
        Ok(())
    }
}

#[allow(dead_code)]
fn assert_runtime_object_safe(runtime: &dyn RuntimeTrait<Base = Plan, Prepared = ()>) {
    let _ = runtime.run();
}
