use std::sync::Arc;

use marix_common::{Actor as ActorTrait, Runtime as RuntimeTrait};
use marix_protocol::{IntentEvent, IntentResult, IntentSignature};

use super::IntentRuntime;
use crate::task::TaskAccess;

#[derive(Clone)]
pub struct Intent {
    pub runtime: Arc<IntentRuntime>,
}

impl ActorTrait for Intent {
    type Signature = IntentSignature;
    type Event = IntentEvent;
    type Result = IntentResult;
    type Runtime = IntentRuntime;

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

impl Intent {
    pub(crate) fn new(
        access: Arc<TaskAccess>,
        signature: IntentSignature,
        content: String,
    ) -> Self {
        let runtime = Arc::new(IntentRuntime::new(access, signature, content));
        Self { runtime }
    }
}

#[allow(dead_code)]
fn assert_actor_object_safe(
    actor: &dyn ActorTrait<
        Signature = IntentSignature,
        Event = IntentEvent,
        Result = IntentResult,
        Runtime = IntentRuntime,
    >,
) {
    actor.start();
}
