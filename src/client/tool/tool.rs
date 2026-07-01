use std::sync::mpsc::{Receiver, Sender};

use super::category::ToolCategory;
use super::error::ToolError;
use crate::common::config::Platform;
use crate::common::external::*;
use crate::common::protocol::{ToolParameter, ToolSchema};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolType {
    Primary,
    Native,
    Skill,
    User,
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolPreview {
    pub name: &'static str,
    pub description: &'static str,
    pub schema: ToolSchema,
}

/// One model-issued request to run a tool, correlated by id so streamed output
/// can be routed back to the originating tool call. Its parameter package is a
/// schema-conformant ToolParameter, so every invocation carries valid arguments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolInvocation {
    pub call_id: String,
    pub name: String,
    pub parameter: ToolParameter,
}

pub struct ToolRuntime {
    pub statuses: Receiver<ToolInvocationStatus>,
    cancel_tx: Sender<()>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolInvocationStatus {
    Started,
    Running(String),
    Cancelled,
    Failed(ToolError),
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolExecutionResult<T> {
    Complete(T),
    Cancelled,
}

impl ToolRuntime {
    pub fn new(statuses: Receiver<ToolInvocationStatus>, cancel_tx: Sender<()>) -> Self {
        Self {
            statuses,
            cancel_tx,
        }
    }

    pub fn cancel(&self) -> Result<(), ToolError> {
        self.cancel_tx.send(()).map_err(|_| {
            ToolError::ExecutionFailed("tool runtime is no longer running".to_string())
        })
    }
}
