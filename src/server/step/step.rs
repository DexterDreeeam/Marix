use std::sync::Arc;

use marix_common::{Actor as ActorTrait, Runtime as RuntimeTrait};
use marix_protocol::{StepDraft, StepEvent, StepResult, StepSignature};

use super::StepRuntime;
use crate::task::TaskAccess;

#[derive(Clone)]
pub struct Step {
    pub runtime: Arc<StepRuntime>,
}

impl ActorTrait for Step {
    type Signature = StepSignature;
    type Event = StepEvent;
    type Result = StepResult;
    type Runtime = StepRuntime;

    fn runtime(&self) -> &Arc<Self::Runtime> {
        &self.runtime
    }

    fn spawn(&self, runtime: Arc<Self::Runtime>) {
        let rt = Arc::clone(&runtime.access.rt);
        drop(rt.spawn(async move {
            runtime.run().await;
        }));
    }
}

// -- Private -- //

impl Step {
    pub(crate) fn from_draft(
        access: Arc<TaskAccess>,
        signature: StepSignature,
        draft: StepDraft,
    ) -> Result<Self, String> {
        if draft.invocations.is_empty() {
            return Err("step must contain an invocation".to_owned());
        }
        if draft
            .invocations
            .iter()
            .any(|invocation| invocation.name.trim().is_empty())
        {
            return Err("step invocation name cannot be empty".to_owned());
        }
        let runtime = Arc::new(StepRuntime::new(access, signature, draft));
        Ok(Self { runtime })
    }
}

#[allow(dead_code)]
fn assert_actor_object_safe(
    actor: &dyn ActorTrait<
        Signature = StepSignature,
        Event = StepEvent,
        Result = StepResult,
        Runtime = StepRuntime,
    >,
) {
    actor.start();
}
