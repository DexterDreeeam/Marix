use std::sync::mpsc::Receiver;

use crate::client::tool::{ToolInvocation, ToolOutcome, ToolRegistry};

use super::error::ExecutorError;

/// Executes tool invocations against a registry. Each invocation streams its
/// output back chunk by chunk over a channel, mirroring the model backend's
/// streaming contract.
pub struct Executor;

impl Executor {
    pub fn new(registry: ToolRegistry) -> Self {
        panic!("not implemented")
    }

    pub fn execute(
        &self,
        invocation: ToolInvocation,
    ) -> Result<Receiver<ToolOutcome>, ExecutorError> {
        panic!("not implemented")
    }
}
