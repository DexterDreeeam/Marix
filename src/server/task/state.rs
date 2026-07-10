use std::fmt;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::external::*;
use marix_common::{AsyncReceiver, AsyncSender, Sender, WorkQueue, build_async_channel};
use marix_protocol::{
    InvocationSignature, PlanSignature, RelaySignature, SessionEvent, StepSignature, TaskEvent,
    TaskSignature,
};

use super::TaskAccess;
use crate::invocation::Invocation;
use crate::plan::Plan;
use crate::relay::Relay;
use crate::session::SessionContext;
use crate::step::Step;

pub struct TaskState {
    pub access: TaskAccess,
    pub plans: Arc<WorkQueue<PlanSignature, Plan>>,
    pub invocations: Arc<WorkQueue<InvocationSignature, Invocation>>,
    pub relays: Arc<WorkQueue<RelaySignature, Relay>>,
    pub steps: Arc<WorkQueue<StepSignature, Step>>,
    pub task_tx: AsyncSender<TaskEvent>,
    pub task_rx: StdMutex<Option<AsyncReceiver<TaskEvent>>>,
}

impl TaskState {
    pub fn new(
        session_context: Arc<StdMutex<SessionContext>>,
        signature: TaskSignature,
        user_request: String,
        session_tx: Sender<SessionEvent>,
    ) -> Self {
        let (task_tx, task_rx) = build_async_channel();
        let plans = Arc::new(WorkQueue::new());
        let invocations = Arc::new(WorkQueue::new());
        let relays = Arc::new(WorkQueue::new());
        let steps = Arc::new(WorkQueue::new());
        let access = TaskAccess::new(
            session_context,
            session_tx,
            signature,
            user_request,
            Arc::clone(&plans),
            Arc::clone(&invocations),
            Arc::clone(&relays),
            Arc::clone(&steps),
        );
        Self {
            access,
            plans,
            invocations,
            relays,
            steps,
            task_tx,
            task_rx: StdMutex::new(Some(task_rx)),
        }
    }
}

// -- Private -- //

impl fmt::Debug for TaskState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TaskState")
            .field("signature", &self.access.signature)
            .finish_non_exhaustive()
    }
}
