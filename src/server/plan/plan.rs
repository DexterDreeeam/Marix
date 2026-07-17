use std::sync::Arc;

use marix_common::{Actor as ActorTrait, Runtime as RuntimeTrait};
use marix_protocol::{IntentSignature, PlanEvent, PlanResult, PlanSignature};

use super::PlanRuntime;
use crate::task::TaskAccess;

#[derive(Clone)]
pub struct Plan {
    pub runtime: Arc<PlanRuntime>,
}

impl ActorTrait for Plan {
    type Signature = PlanSignature;
    type Event = PlanEvent;
    type Result = PlanResult;
    type Runtime = PlanRuntime;

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

impl Plan {
    pub(crate) fn new(
        access: Arc<TaskAccess>,
        signature: PlanSignature,
        intents: Vec<IntentSignature>,
    ) -> Self {
        let runtime = Arc::new(PlanRuntime::new(access, signature, intents));
        Self { runtime }
    }
}

#[allow(dead_code)]
fn assert_actor_object_safe(
    actor: &dyn ActorTrait<
        Signature = PlanSignature,
        Event = PlanEvent,
        Result = PlanResult,
        Runtime = PlanRuntime,
    >,
) {
    actor.start();
}
