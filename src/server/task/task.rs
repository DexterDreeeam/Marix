use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::thread;

use marix_common::{Actor as ActorTrait, ActorStatus, Runtime as RuntimeTrait, Sender};
use marix_protocol::{
    SessionEvent, TaskEvent, TaskPreview, TaskRequest, TaskRequestBrief, TaskResult, TaskSignature,
};

use super::TaskRuntime;
use crate::session::SessionContext;

const COMPLETION_TIME_EXCEEDED: &str = "maximum completion time exceeded";

#[derive(Clone)]
pub struct Task {
    pub runtime: Arc<TaskRuntime>,
}

impl Task {
    pub fn preview(&self) -> TaskPreview {
        TaskPreview {
            request: TaskRequestBrief {
                content: self.runtime.access.user_request.clone(),
            },
            result: self.result(),
        }
    }
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
        let watchdog = runtime.access.completion_deadline().map(|deadline| {
            let watchdog_runtime = Arc::clone(&runtime);
            rt.spawn(async move {
                tokio::time::sleep_until(deadline.into()).await;
                if matches!(watchdog_runtime.status(), ActorStatus::Complete(_)) {
                    return;
                }
                watchdog_runtime.fail_task(COMPLETION_TIME_EXCEEDED.to_owned());
            })
        });
        drop(thread::spawn(move || {
            rt.block_on(runtime.run());
            if let Some(watchdog) = watchdog {
                watchdog.abort();
                let _ = rt.block_on(watchdog);
            }
        }));
    }
}

// -- Private -- //

impl Task {
    pub(crate) fn new(
        session_context: Arc<StdMutex<SessionContext>>,
        request: TaskRequest,
        session_tx: Sender<SessionEvent>,
    ) -> Self {
        let runtime = Arc::new(TaskRuntime::new(session_context, request, session_tx));
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
