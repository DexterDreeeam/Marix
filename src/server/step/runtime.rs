use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::{
    Actor, ActorPrepareFuture, ActorRuntime as ActorRuntimeTrait, ActorStatus, Lifecycle, Logger,
};
use marix_protocol::{
    IntentEvent, InvocationEvent, InvocationRequest, InvocationResultKind, InvocationSignature,
    SessionEvent, StepDraft, StepEvent, StepResult, StepResultKind, StepSignature, TaskEvent,
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
}

impl StepRuntime {
    pub(crate) fn new(access: Arc<TaskAccess>, signature: StepSignature, draft: StepDraft) -> Self {
        Self {
            access,
            signature,
            draft,
            invocations: StdMutex::new(Vec::new()),
            lifecycle: Lifecycle::new(),
        }
    }
}

impl ActorRuntimeTrait for StepRuntime {
    type Base = Step;
    type Prepared = ();

    fn signature(&self) -> &StepSignature {
        &self.signature
    }

    fn lifecycle(&self) -> &Lifecycle<StepEvent, StepResult> {
        &self.lifecycle
    }

    fn prepare(&self) -> ActorPrepareFuture<'_, Self::Prepared> {
        Box::pin(async move {
            let actors = match self.create_invocations() {
                Ok(actors) => actors,
                Err(reason) => {
                    self.finish(StepResultKind::Failed, reason);
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

    fn on_finish(&self) {
        self.send_intent_update(ActorStatus::Complete);
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
            if !self.access.insert_invocation(actor.clone()) {
                return Err(format!("invocation {signature} is duplicated",));
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

    fn on_invocation_update(&self, signature: InvocationSignature, status: ActorStatus) {
        if self.status().is_terminal() {
            Logger::error(format!(
                "step {} received invocation {signature} update \
                 {status:?} after completion",
                &self.signature,
            ));
            return;
        }
        if !status.is_terminal() {
            return;
        }
        let Some(result) = self.access.get_invocation_result(&signature) else {
            self.finish(
                StepResultKind::Failed,
                format!(
                    "invocation {signature} completed without a \
                     result",
                ),
            );
            return;
        };
        match result.kind {
            InvocationResultKind::Succeed => {
                self.finish_if_complete();
            }
            InvocationResultKind::Canceled => {
                self.finish(StepResultKind::Canceled, result.output);
            }
            InvocationResultKind::Failed => {
                self.finish(StepResultKind::Failed, result.output);
            }
        }
    }

    fn finish_if_complete(&self) {
        let invocations = self
            .invocations
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        let mut outputs = Vec::with_capacity(invocations.len());
        for signature in &invocations {
            let Some(result) = self.access.get_invocation_result(signature) else {
                return;
            };
            match result.kind {
                InvocationResultKind::Succeed => {
                    outputs.push(format!("{}: {}", &signature.name, result.output,));
                }
                InvocationResultKind::Canceled => {
                    self.finish(StepResultKind::Canceled, result.output);
                    return;
                }
                InvocationResultKind::Failed => {
                    self.finish(StepResultKind::Failed, result.output);
                    return;
                }
            }
        }
        self.finish(StepResultKind::Succeed, outputs.join("\n"));
    }

    fn cancel(&self) {
        if self.status().is_terminal() {
            return;
        }
        let invocations = self
            .invocations
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        for signature in &invocations {
            if self.access.get_invocation_result(signature).is_some() {
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
        }
        self.finish(StepResultKind::Canceled, "step canceled".to_owned());
    }

    fn finish(&self, kind: StepResultKind, output: String) {
        ActorRuntimeTrait::finish(self, StepResult { kind, output });
    }

    fn send_intent_update(&self, status: ActorStatus) {
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
fn assert_runtime_object_safe(runtime: &dyn ActorRuntimeTrait<Base = Step, Prepared = ()>) {
    let _ = runtime.run();
}
