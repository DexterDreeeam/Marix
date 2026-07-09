use std::fmt;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::external::*;
use marix_common::{Sender, WorkQueue, build_async_channel};
use marix_protocol::{SessionEvent, StepId, TaskEvent, TaskSignature};

use super::TaskAccess;
use crate::invocation::InvocationHub;
use crate::plan::PlanHub;
use crate::relay::RelayHub;
use crate::session::SessionContext;
use crate::step::Step;

pub struct TaskState {
    pub access: TaskAccess,
    pub plan_hub: Arc<PlanHub>,
    pub invocation_hub: Arc<InvocationHub>,
    pub relay_hub: Arc<RelayHub>,
    pub steps: Arc<WorkQueue<StepId, Step>>,
    pub task_tx: tokio::mpsc::UnboundedSender<TaskEvent>,
    pub task_rx: StdMutex<Option<tokio::mpsc::UnboundedReceiver<TaskEvent>>>,
}

impl TaskState {
    pub fn new(
        session_context: Arc<StdMutex<SessionContext>>,
        signature: TaskSignature,
        user_request: String,
        session_tx: Sender<SessionEvent>,
    ) -> Self {
        let (task_tx, task_rx) = build_async_channel();
        let rt = tokio::Builder::new_multi_thread()
            .enable_all()
            .thread_name("marix-task-runtime")
            .build()
            .unwrap_or_else(|error| panic!("failed to build task runtime: {error}"));
        let access = TaskAccess {
            session_context,
            session_tx,
            signature,
            user_request,
            rt: Arc::new(rt),
        };
        Self {
            access,
            plan_hub: Arc::new(PlanHub::new()),
            invocation_hub: Arc::new(InvocationHub::new()),
            relay_hub: Arc::new(RelayHub::new()),
            steps: Arc::new(WorkQueue::new()),
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
