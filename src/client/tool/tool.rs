use std::sync::mpsc::Receiver;

use super::descriptor::ToolDescriptor;
use super::error::ToolError;
use crate::common::external::*;

/// A callable client-side capability (shell, network, filesystem, ...). Both
/// built-in and user-provided tools implement this single trait so the registry
/// can treat them uniformly. Invocation streams output chunks over a channel.
pub trait Tool {
    fn descriptor(&self) -> ToolDescriptor;

    fn invoke(&self, invocation: ToolInvocation) -> Result<Receiver<ToolOutcome>, ToolError>;
}

/// One model-issued request to run a tool, correlated by id so streamed output
/// can be routed back to the originating tool call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolInvocation {
    pub call_id: String,
    pub name: String,
    pub arguments: String,
}

/// A single streamed chunk of a tool's output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolOutput {
    pub call_id: String,
    pub content: String,
}

/// One item in a tool's output stream: an output chunk or a terminal failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolOutcome {
    Output(ToolOutput),
    Error(ToolError),
}
