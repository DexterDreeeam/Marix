use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::{
    AsyncReceiver, AsyncSender, Sender, WorkQueue, build_async_channel,
};
use marix_protocol::{
    IntentSignature, InvocationSignature, PlanSignature,
    RelaySignature, SessionEvent, StepSignature, TaskEvent,
    TaskResult, TaskSignature, TaskStatus,
};

use super::TaskAccess;
use crate::intent::Intent;
use crate::invocation::Invocation;
use crate::plan::Plan;
use crate::relay::Relay;
use crate::session::SessionContext;
use crate::step::Step;

pub struct TaskState {
    pub access: Arc<TaskAccess>,
    pub intents: Arc<WorkQueue<IntentSignature, Intent>>,
    pub plans: Arc<WorkQueue<PlanSignature, Plan>>,
    pub steps: Arc<WorkQueue<StepSignature, Step>>,
    pub invocations: Arc<WorkQueue<InvocationSignature, Invocation>>,
    pub relays: Arc<WorkQueue<RelaySignature, Relay>>,
    pub root: IntentSignature,
    pub status: StdMutex<TaskStatus>,
    pub result: StdMutex<Option<TaskResult>>,
    pub task_tx: AsyncSender<TaskEvent>,
    pub task_rx: StdMutex<Option<AsyncReceiver<TaskEvent>>>,
}

// -- Private -- //

impl TaskState {
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
        let (task_tx, task_rx) = build_async_channel();
        Self {
            access,
            intents,
            plans,
            steps,
            invocations,
            relays,
            root,
            status: StdMutex::new(TaskStatus::Created),
            result: StdMutex::new(None),
            task_tx,
            task_rx: StdMutex::new(Some(task_rx)),
        }
    }
}
