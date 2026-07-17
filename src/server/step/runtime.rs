use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::{
    Actor, ActorStartFuture, ActorStatus, Lifecycle, Logger, Runtime as RuntimeTrait,
};
use marix_protocol::{
    IntentEvent, InvocationEvent, InvocationRequest, InvocationResult, InvocationResultKind,
    InvocationSignature, SessionEvent, StepDraft, StepEvent, StepResult, StepResultKind,
    StepSignature, TaskEvent, ToolCallResultDraft,
};

use super::Step;
use crate::invocation::Invocation;
use crate::task::TaskAccess;

pub struct StepRuntime {
    pub access: Arc<TaskAccess>,
    pub signature: StepSignature,
    pub draft: StepDraft,
    pub invocations: StdMutex<Vec<InvocationSignature>>,
    pub lifecycle: Lifecycle<StepEvent, StepResult>,
    invocation_results: StdMutex<BTreeMap<InvocationSignature, InvocationResult>>,
}

impl StepRuntime {
    pub(crate) fn new(access: Arc<TaskAccess>, signature: StepSignature, draft: StepDraft) -> Self {
        Self {
            access,
            signature,
            draft,
            invocations: StdMutex::new(Vec::new()),
            lifecycle: Lifecycle::new(),
            invocation_results: StdMutex::new(BTreeMap::new()),
        }
    }
}

impl RuntimeTrait for StepRuntime {
    type Base = Step;
    type Prepared = ();

    fn signature(&self) -> &StepSignature {
        &self.signature
    }

    fn lifecycle(&self) -> &Lifecycle<StepEvent, StepResult> {
        &self.lifecycle
    }

    fn on_start(&self) -> ActorStartFuture<'_, Self::Prepared> {
        Box::pin(async move {
            let actors = match self.create_invocations() {
                Ok(actors) => actors,
                Err(reason) => {
                    Logger::error(reason);
                    self.finish(StepResult {
                        kind: StepResultKind::Failed,
                        calls: Vec::new(),
                    });
                    return None;
                }
            };
            for actor in actors {
                actor.start();
            }
            Some(())
        })
    }

    fn dispatch(&self, event: StepEvent) {
        match event {
            StepEvent::Update(signature, status) => {
                self.on_invocation_update(signature, status);
            }
            StepEvent::Cancel => self.cancel(),
        }
    }

    fn on_finish(&self, result: StepResult) {
        self.send_intent_update(ActorStatus::Complete(result));
    }
}

// -- Private -- //

impl StepRuntime {
    fn create_invocations(&self) -> Result<Vec<Invocation>, String> {
        let drafts = self.draft.invocations.clone();
        let mut actors = Vec::with_capacity(drafts.len());
        let mut signatures = Vec::with_capacity(drafts.len());
        for draft in drafts {
            let signature = InvocationSignature::new(self.signature.clone(), draft.name);
            let request = InvocationRequest {
                signature: signature.clone(),
                input: draft.input,
            };
            let actor = Invocation::new(Arc::clone(&self.access), request);
            if !self.access.insert(actor.clone()) {
                return Err(format!("invocation {signature} is duplicated"));
            }
            actors.push(actor);
            signatures.push(signature);
        }
        *self
            .invocations
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = signatures;
        Ok(actors)
    }

    fn on_invocation_update(
        &self,
        signature: InvocationSignature,
        status: ActorStatus<InvocationResult>,
    ) {
        if matches!(self.status(), ActorStatus::Complete(_)) {
            Logger::error(format!(
                "step {} received invocation {signature} update \
                 {status:?} after completion",
                &self.signature,
            ));
            return;
        }
        let ActorStatus::Complete(result) = status else {
            return;
        };
        let known = self
            .invocations
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .contains(&signature);
        if !known {
            Logger::error(format!(
                "step {} received update from unknown invocation \
                 {signature}",
                &self.signature,
            ));
            return;
        }
        let mut results = self
            .invocation_results
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if results.contains_key(&signature) {
            Logger::error(format!(
                "step {} received duplicate complete update from \
                 invocation {signature}",
                &self.signature,
            ));
            return;
        }
        results.insert(signature, result);
        drop(results);
        self.finish_if_complete();
    }

    fn finish_if_complete(&self) {
        let invocations = self
            .invocations
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        let mut calls = Vec::with_capacity(invocations.len());
        let mut failed = false;
        let mut canceled = false;
        let results = self
            .invocation_results
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        for signature in &invocations {
            let Some(result) = results.get(signature).cloned() else {
                return;
            };
            match &result.kind {
                InvocationResultKind::Succeed => {}
                InvocationResultKind::Canceled => canceled = true,
                InvocationResultKind::Failed => failed = true,
            }
            calls.push(ToolCallResultDraft {
                tool: signature.name.clone(),
                result,
            });
        }
        drop(results);
        let kind = if failed {
            StepResultKind::Failed
        } else if canceled {
            StepResultKind::Canceled
        } else {
            StepResultKind::Succeed
        };
        self.finish(StepResult { kind, calls });
    }

    fn cancel(&self) {
        if matches!(self.status(), ActorStatus::Complete(_)) {
            return;
        }
        let invocations = self
            .invocations
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        let results = self
            .invocation_results
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        let mut calls = Vec::with_capacity(invocations.len());
        for signature in &invocations {
            if let Some(result) = results.get(signature).cloned() {
                calls.push(ToolCallResultDraft {
                    tool: signature.name.clone(),
                    result,
                });
                continue;
            }
            let event = SessionEvent::Task(
                self.access.signature.clone(),
                TaskEvent::Invocation(signature.clone(), InvocationEvent::Cancel),
            );
            if self.access.session_tx.send(event).is_err() {
                Logger::warning(format!(
                    "step {} invocation {signature} cancel failed: \
                     session stopped",
                    &self.signature,
                ));
            }
            calls.push(ToolCallResultDraft {
                tool: signature.name.clone(),
                result: InvocationResult {
                    kind: InvocationResultKind::Canceled,
                    output: "tool call canceled".to_owned(),
                    seq_count: 0,
                },
            });
        }
        self.finish(StepResult {
            kind: StepResultKind::Canceled,
            calls,
        });
    }

    fn finish(&self, result: StepResult) {
        RuntimeTrait::finish(self, result);
    }

    fn send_intent_update(&self, status: ActorStatus<StepResult>) {
        let intent = self.signature.intent.clone();
        let event = SessionEvent::Task(
            intent.task.clone(),
            TaskEvent::Intent(
                intent,
                IntentEvent::StepUpdate(self.signature.clone(), status),
            ),
        );
        if self.access.session_tx.send(event).is_err() {
            Logger::warning(format!(
                "step {} update failed: session stopped",
                &self.signature,
            ));
        }
    }
}

#[allow(dead_code)]
fn assert_runtime_object_safe(runtime: &dyn RuntimeTrait<Base = Step, Prepared = ()>) {
    let _ = runtime.run();
}
