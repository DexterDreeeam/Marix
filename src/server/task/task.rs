use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::thread;

use marix_common::{Actor as ActorTrait, Runtime as RuntimeTrait, Sender};
use marix_protocol::{IntentSignature, SessionEvent, TaskEvent, TaskResult, TaskSignature};

use super::TaskRuntime;
use crate::session::SessionContext;

#[derive(Clone)]
pub struct Task {
    pub runtime: Arc<TaskRuntime>,
}

impl ActorTrait for Task {
    type Signature = TaskSignature;
    type Event = TaskEvent;
    type Result = TaskResult;
    type Runtime = TaskRuntime;

    fn runtime(&self) -> &Arc<Self::Runtime> {
        &self.runtime
    }

    fn spawn(&self, runtime: Arc<Self::Runtime>) {
        let rt = Arc::clone(&runtime.access.rt);
        drop(thread::spawn(move || {
            rt.block_on(runtime.run());
        }));
    }
}

// -- Private -- //

impl Task {
    pub(crate) fn new(
        session_context: Arc<StdMutex<SessionContext>>,
        signature: TaskSignature,
        root: IntentSignature,
        user_request: String,
        session_tx: Sender<SessionEvent>,
    ) -> Self {
        let runtime = Arc::new(TaskRuntime::new(
            session_context,
            signature,
            root,
            user_request,
            session_tx,
        ));
        Self { runtime }
    }
}

#[allow(dead_code)]
fn assert_actor_object_safe(
    actor: &dyn ActorTrait<
        Signature = TaskSignature,
        Event = TaskEvent,
        Result = TaskResult,
        Runtime = TaskRuntime,
    >,
) {
    actor.start();
}
