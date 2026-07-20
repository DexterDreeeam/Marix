use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::time::{Duration, Instant};

use marix_common::{
    Actor, ActorStartFuture, ActorStatus, Lifecycle, Logger, Runtime as RuntimeTrait, Sender,
    WorkQueue,
};
use marix_protocol::{
    IntentEvent, IntentResult, IntentResultKind, IntentSignature, InvocationEvent,
    InvocationSignature, RelayEvent, RelaySignature, SessionEvent, StepEvent, StepSignature,
    TaskEvent, TaskRequest, TaskResult, TaskResultKind, TaskSignature, TaskStatus,
};

use super::{Task, TaskAccess};
use crate::intent::Intent;
use crate::invocation::Invocation;
use crate::relay::Relay;
use crate::session::SessionContext;
use crate::step::Step;

const MIN_COMPLETION_TIME_SECS: u64 = 10;
const MIN_RELAY_COUNT: u64 = 5;

pub struct TaskRuntime {
    pub access: Arc<TaskAccess>,
    pub intents: Arc<WorkQueue<IntentSignature, Intent>>,
    pub steps: Arc<WorkQueue<StepSignature, Step>>,
    pub invocations: Arc<WorkQueue<InvocationSignature, Invocation>>,
    pub relays: Arc<WorkQueue<RelaySignature, Relay>>,
    pub root: IntentSignature,
    pub lifecycle: Lifecycle<TaskEvent, TaskResult>,
}

impl TaskRuntime {
    pub(crate) fn new(
        session_context: Arc<StdMutex<SessionContext>>,
        request: TaskRequest,
        session_tx: Sender<SessionEvent>,
    ) -> Self {
        let TaskRequest {
            signature,
            content,
            max_completion_time_secs,
            max_relay_count,
        } = request;
        let root = IntentSignature::new(signature.clone(), None, "root".to_owned());
        let deadline = max_completion_time_secs
            .map(|seconds| Duration::from_secs(seconds.max(MIN_COMPLETION_TIME_SECS)))
            .and_then(|duration| Instant::now().checked_add(duration));
        let left_relay = max_relay_count
            .map(|count| usize::try_from(count.max(MIN_RELAY_COUNT)).unwrap_or(usize::MAX));
        let intents = Arc::new(WorkQueue::new());
        let steps = Arc::new(WorkQueue::new());
        let invocations = Arc::new(WorkQueue::new());
        let relays = Arc::new(WorkQueue::new());
        let access = TaskAccess::new(
            session_context,
            session_tx,
            signature,
            content,
            deadline,
            left_relay,
            Arc::clone(&intents),
            Arc::clone(&steps),
            Arc::clone(&invocations),
            Arc::clone(&relays),
        );
        Self {
            access,
            intents,
            steps,
            invocations,
            relays,
            root,
            lifecycle: Lifecycle::new(),
        }
    }
}

impl RuntimeTrait for TaskRuntime {
    type Base = Task;
    type Prepared = ();

    fn signature(&self) -> &TaskSignature {
        &self.access.signature
    }

    fn lifecycle(&self) -> &Lifecycle<TaskEvent, TaskResult> {
        &self.lifecycle
    }

    fn on_start(&self) -> ActorStartFuture<'_, Self::Prepared> {
        Box::pin(async move {
            self.send_session_status(TaskStatus::Started);
            Logger::log(format!("task {} started", &self.access.signature,));
            let root = Intent::new(
                Arc::clone(&self.access),
                self.root.clone(),
                self.access.user_request.clone(),
            );
            if !self.access.insert(root.clone()) {
                self.fail_task("root intent already exists".to_owned());
                return None;
            }
            root.start();
            Some(())
        })
    }

    fn dispatch(&self, event: TaskEvent) {
        self.route(event);
    }

    fn on_finish(&self, result: TaskResult) {
        let status = match result.kind.clone() {
            TaskResultKind::Succeed => TaskStatus::Succeed(result),
            TaskResultKind::Canceled => TaskStatus::Canceled,
            TaskResultKind::Failed => TaskStatus::Failed {
                reason: result.output,
            },
        };
        self.send_session_status(status);
    }
}

// -- Private -- //

impl TaskRuntime {
    pub(super) fn on_root_update(
        &self,
        signature: IntentSignature,
        status: ActorStatus<IntentResult>,
    ) {
        let ActorStatus::Complete(result) = status else {
            return;
        };
        if signature != self.root {
            self.fail_task(format!(
                "root update came from unexpected intent {signature}",
            ));
            return;
        }
        self.finish_root(result);
    }

    fn finish_root(&self, result: IntentResult) {
        let IntentResult { kind, output } = result;
        let result = match kind {
            IntentResultKind::Succeed => TaskResult {
                kind: TaskResultKind::Succeed,
                output,
            },
            IntentResultKind::Canceled => TaskResult {
                kind: TaskResultKind::Canceled,
                output,
            },
            IntentResultKind::Infeasible => TaskResult {
                kind: TaskResultKind::Failed,
                output: format!("root intent infeasible: {output}"),
            },
            IntentResultKind::Failed => TaskResult {
                kind: TaskResultKind::Failed,
                output,
            },
        };
        RuntimeTrait::finish(self, result);
    }

    pub(super) fn cancel_task(&self) {
        if matches!(self.status(), ActorStatus::Complete(_)) {
            return;
        }
        self.cancel_all();
        RuntimeTrait::finish(
            self,
            TaskResult {
                kind: TaskResultKind::Canceled,
                output: "task canceled".to_owned(),
            },
        );
    }

    pub(super) fn fail_task(&self, reason: String) {
        Logger::error(format!("task {} failed: {reason}", &self.access.signature,));
        self.cancel_all();
        self.finish_failed(reason);
    }

    fn cancel_all(&self) {
        for relay in self.relays.list() {
            if !matches!(relay.status(), ActorStatus::Complete(_)) {
                RuntimeTrait::dispatch(relay.runtime.as_ref(), RelayEvent::Cancel);
            }
        }
        for invocation in self.invocations.list() {
            if !matches!(invocation.status(), ActorStatus::Complete(_)) {
                RuntimeTrait::dispatch(invocation.runtime.as_ref(), InvocationEvent::Cancel);
            }
        }
        for step in self.steps.list() {
            if !matches!(step.status(), ActorStatus::Complete(_)) {
                RuntimeTrait::dispatch(step.runtime.as_ref(), StepEvent::Cancel);
            }
        }
        for intent in self.intents.list() {
            if !matches!(intent.status(), ActorStatus::Complete(_)) {
                RuntimeTrait::dispatch(intent.runtime.as_ref(), IntentEvent::Cancel);
            }
        }
    }

    fn finish_failed(&self, reason: String) {
        RuntimeTrait::finish(
            self,
            TaskResult {
                kind: TaskResultKind::Failed,
                output: reason,
            },
        );
    }

    fn send_session_status(&self, status: TaskStatus) {
        if self
            .access
            .session_tx
            .send(SessionEvent::TaskUpdate(status))
            .is_err()
        {
            Logger::warning(format!(
                "task {} status update failed: session stopped",
                &self.access.signature,
            ));
        }
    }
}

#[allow(dead_code)]
fn assert_runtime_object_safe(runtime: &dyn RuntimeTrait<Base = Task, Prepared = ()>) {
    let _ = runtime.run();
}
