use std::sync::mpsc::Receiver;

use super::category::ToolCategory;
use super::error::ToolError;
use crate::common::external::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolType {
    Primary,
    Native,
    Skill,
    User,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolPlatform {
    All,
    // Minimum supported version: Windows 10 22H2.
    Win,
    // Minimum supported version: Ubuntu 22.04 LTS.
    Ubuntu,
}

/// A callable client-side capability (shell, network, filesystem, ...). Both
/// built-in and user-provided tools implement this single trait so the registry
/// can treat them uniformly. Invocation streams output chunks over a channel.
pub trait Tool {
    fn tool_type(&self) -> ToolType;

    fn platforms(&self) -> &'static [ToolPlatform];

    fn categories(&self) -> &'static [ToolCategory];

    fn name(&self) -> &'static str;

    fn description(&self) -> &'static str;

    fn schema(&self) -> &'static str;

    fn invoke(&self, invocation: ToolInvocation) -> Result<Receiver<ToolOutcome>, ToolError>;
}

pub trait UserTool: Tool {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolPreview {
    pub name: &'static str,
    pub description: &'static str,
    pub schema: &'static str,
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
