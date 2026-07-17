use std::sync::Arc;

use marix_common::{Actor as ActorTrait, ActorRuntime as ActorRuntimeTrait};
use marix_protocol::{RelayEvent, RelayRequest, RelayResult, RelaySignature};

use super::RelayRuntime;
use crate::task::TaskAccess;

#[derive(Clone)]
pub struct Relay {
    pub runtime: Arc<RelayRuntime>,
}

impl ActorTrait for Relay {
    type Signature = RelaySignature;
    type Event = RelayEvent;
    type Result = RelayResult;
    type Runtime = RelayRuntime;

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

impl Relay {
    pub(crate) fn new(access: Arc<TaskAccess>, request: RelayRequest) -> Result<Self, String> {
        let runtime = Arc::new(RelayRuntime::new(access, request)?);
        Ok(Self { runtime })
    }
}

#[allow(dead_code)]
fn assert_actor_object_safe(
    actor: &dyn ActorTrait<
        Signature = RelaySignature,
        Event = RelayEvent,
        Result = RelayResult,
        Runtime = RelayRuntime,
    >,
) {
    actor.start();
}
