use std::collections::HashMap;

use crate::client::tool::{ToolInvocation, ToolRegistry, ToolRuntime};
use crate::common::protocol::{ToolExecutionStatus, ToolSignature};

use super::error::ExecutorError;

/// Executes tool invocations against a registry. Each invocation streams its
/// output back chunk by chunk over a channel, mirroring the model backend's
/// streaming contract.
pub struct Executor {
    registry: ToolRegistry,
    runtimes: HashMap<String, ToolRuntime>,
}

impl Executor {
    pub fn new(_registry: ToolRegistry) -> Self {
        panic!("not implemented")
    }

    pub fn invoke(&mut self, _invocation: ToolInvocation) -> Result<(), ExecutorError> {
        panic!("not implemented")
    }

    pub fn status(&self, _signature: ToolSignature) -> Result<ToolExecutionStatus, ExecutorError> {
        panic!("not implemented")
    }

    pub fn cancel(&mut self, _signature: ToolSignature) -> Result<(), ExecutorError> {
        panic!("not implemented")
    }
}
