use std::sync::Arc;

use marix_common::{Actor as ActorTrait, ActorRuntime as ActorRuntimeTrait};
use marix_protocol::{InvocationEvent, InvocationRequest, InvocationResult, InvocationSignature};

use super::InvocationRuntime;
use crate::task::TaskAccess;

#[derive(Clone)]
pub struct Invocation {
    pub runtime: Arc<InvocationRuntime>,
}

impl ActorTrait for Invocation {
    type Signature = InvocationSignature;
    type Event = InvocationEvent;
    type Result = InvocationResult;
    type Runtime = InvocationRuntime;

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

impl Invocation {
    pub(crate) fn new(access: Arc<TaskAccess>, request: InvocationRequest) -> Self {
        let runtime = Arc::new(InvocationRuntime::new(
            access,
            request.signature,
            request.input,
        ));
        Self { runtime }
    }
}

#[allow(dead_code)]
fn assert_actor_object_safe(
    actor: &dyn ActorTrait<
        Signature = InvocationSignature,
        Event = InvocationEvent,
        Result = InvocationResult,
        Runtime = InvocationRuntime,
    >,
) {
    actor.start();
}
