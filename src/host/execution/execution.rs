use std::sync::{Arc, Mutex};
use std::thread;

use crate::executor::{ExecutorCache, Tool};
use marix_common::external::*;
use marix_common::{Actor as ActorTrait, Logger, Runtime as RuntimeTrait, SharedNetSender};
use marix_protocol::{
    ExecutionEvent, ExecutionRequest, ExecutionResult, ExecutionSignature, SessionMessage,
};

use super::ExecutionRuntime;

#[derive(Clone)]
pub struct Execution {
    pub runtime: Arc<ExecutionRuntime>,
}

impl Execution {
    pub(crate) fn new(
        tool: Tool,
        request: ExecutionRequest,
        server_tx: SharedNetSender<SessionMessage>,
        cache: Arc<Mutex<ExecutorCache>>,
    ) -> Self {
        Self {
            runtime: Arc::new(ExecutionRuntime::new(tool, request, server_tx, cache)),
        }
    }
}

impl ActorTrait for Execution {
    type Signature = ExecutionSignature;
    type Event = ExecutionEvent;
    type Result = ExecutionResult;
    type Runtime = ExecutionRuntime;

    fn runtime(&self) -> &Arc<Self::Runtime> {
        &self.runtime
    }

    fn spawn(&self, runtime: Arc<Self::Runtime>) {
        drop(thread::spawn(move || {
            let rt = match tokio::Builder::new_current_thread().enable_all().build() {
                Ok(rt) => rt,
                Err(error) => {
                    Logger::error(format!(
                        "execution {} runtime build failed: {error}",
                        runtime.signature(),
                    ));
                    return;
                }
            };
            rt.block_on(runtime.run());
        }));
    }
}
