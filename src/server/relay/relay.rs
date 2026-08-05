use std::sync::Arc;

use marix_common::{Actor as ActorTrait, Runtime as RuntimeTrait};
use marix_protocol::{
    InvocationSignature, RelayEvent, RelayResult, RelaySignature,
};

use super::RelayRuntime;
use crate::stage::StageAssembler;
use crate::task::{TaskAccess, TaskGate};

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
    pub(crate) fn new(
        access: Arc<TaskAccess>,
        signature: RelaySignature,
        assembler: StageAssembler,
        invocation_owner: Option<InvocationSignature>,
    ) -> Result<Self, String> {
        let stage_type = assembler.stage_type();
        match (stage_type.is_intent(), invocation_owner.as_ref()) {
            (true, Some(owner)) => {
                return Err(format!(
                    "intent stage {stage_type:?} unexpectedly carries \
                     invocation owner {owner}"
                ));
            }
            (false, None) => {
                return Err(format!(
                    "invocation stage {stage_type:?} has no invocation \
                     owner"
                ));
            }
            _ => {}
        }
        access.gate(TaskGate::Relay)?;
        let runtime = Arc::new(RelayRuntime::new(
            access,
            signature,
            assembler,
            invocation_owner,
        )?);
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
