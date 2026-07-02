use super::category::ToolCategory;
use crate::common::config::Platform;
use crate::common::protocol::{
    ToolError, ToolInvocation, ToolExecutionStatus, ToolPreview, ToolSchema, ToolType,
};
use std::sync::{mpsc, Arc, Mutex};

/// A callable client-side capability (shell, network, filesystem, ...). Both
/// built-in and user-provided tools implement this single trait so the registry
/// can treat them uniformly. Invocation streams output chunks over a channel.
pub trait Tool {
    fn tool_type(&self) -> ToolType;

    fn platform(&self) -> Platform;

    fn category(&self) -> ToolCategory;

    fn name(&self) -> &'static str;

    fn description(&self) -> &'static str;

    fn schema(&self) -> ToolSchema;

    fn preview(&self) -> ToolPreview {
        ToolPreview {
            name: self.name(),
            description: self.description(),
            schema: self.schema(),
        }
    }

    fn invoke(&self, invocation: ToolInvocation) -> Result<ToolRuntime, ToolError>;
}

pub trait UserTool: Tool {}

pub struct ToolRuntime {
    pub statuses: mpsc::Receiver<ToolExecutionStatus>,
    pub status_tx: mpsc::Sender<ToolExecutionStatus>,
    pub cancel_rx: mpsc::Receiver<()>,
    cancel_tx: mpsc::Sender<()>,
    status: Arc<Mutex<ToolExecutionStatus>>,
}

impl ToolRuntime {
    pub fn new() -> Self {
        let (status_tx, statuses) = mpsc::channel();
        let (cancel_tx, cancel_rx) = mpsc::channel();
        Self {
            statuses,
            status_tx,
            cancel_rx,
            cancel_tx,
            status: Arc::new(Mutex::new(ToolExecutionStatus::Pending)),
        }
    }

    pub fn status(&self) -> ToolExecutionStatus {
        panic!("not implemented")
    }

    pub fn sync_status(&self, status: ToolExecutionStatus) {
        panic!("not implemented")
    }

    pub fn cancel(&self) -> Result<(), ToolError> {
        self.cancel_tx.send(()).map_err(|_| {
            ToolError::ExecutionFailed("tool runtime is no longer running".to_string())
        })
    }
}
