use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::{
    Actor, ActorStartFuture, ActorStatus, Lifecycle, Logger, Runtime as RuntimeTrait, Sender,
    WorkQueue,
};
use marix_protocol::{
    IntentEvent, IntentResult, IntentResultKind, IntentSignature, InvocationSignature,
    PlanSignature, RelaySignature, SessionEvent, StepSignature, TaskEvent, TaskResult,
    TaskResultKind, TaskSignature, TaskStatus,
};

use super::{Task, TaskAccess};
use crate::intent::Intent;
use crate::invocation::Invocation;
use crate::plan::Plan;
use crate::relay::Relay;
use crate::session::SessionContext;
use crate::step::Step;

pub struct TaskRuntime {
    pub access: Arc<TaskAccess>,
    pub intents: Arc<WorkQueue<IntentSignature, Intent>>,
    pub plans: Arc<WorkQueue<PlanSignature, Plan>>,
    pub steps: Arc<WorkQueue<StepSignature, Step>>,
    pub invocations: Arc<WorkQueue<InvocationSignature, Invocation>>,
    pub relays: Arc<WorkQueue<RelaySignature, Relay>>,
    pub root: IntentSignature,
    pub lifecycle: Lifecycle<TaskEvent, TaskResult>,
}

impl TaskRuntime {
    pub(crate) fn new(
        session_context: Arc<StdMutex<SessionContext>>,
        signature: TaskSignature,
        root: IntentSignature,
        user_request: String,
        session_tx: Sender<SessionEvent>,
    ) -> Self {
        let intents = Arc::new(WorkQueue::new());
        let plans = Arc::new(WorkQueue::new());
        let steps = Arc::new(WorkQueue::new());
        let invocations = Arc::new(WorkQueue::new());
        let relays = Arc::new(WorkQueue::new());
        let access = TaskAccess::new(
            session_context,
            session_tx,
            signature,
            user_request,
            Arc::clone(&intents),
            Arc::clone(&plans),
            Arc::clone(&steps),
            Arc::clone(&invocations),
            Arc::clone(&relays),
        );
        Self {
            access,
            intents,
            plans,
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

    fn on_finish(&self) {
        let Some(result) = self.lifecycle.result() else {
            Logger::error(format!(
                "task {} completed without a result",
                &self.access.signature,
            ));
            return;
        };
        let status = match result.kind {
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
    pub(super) fn on_root_update(&self, signature: IntentSignature, status: ActorStatus) {
        if status != ActorStatus::Complete {
            return;
        }
        if signature != self.root {
            self.fail_task(format!(
                "root update came from unexpected intent {signature}",
            ));
            return;
        }
        let Some(result) = self.access.get_result(&signature) else {
            self.fail_task(format!(
                "root intent {signature} completed without a result",
            ));
            return;
        };
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
        if self.status() == ActorStatus::Complete {
            return;
        }
        for intent in self.intents.list() {
            if intent.status() != ActorStatus::Complete {
                intent.dispatch(IntentEvent::Cancel);
            }
        }
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
