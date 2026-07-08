use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use marix_common::Logger;
use marix_protocol::{
    ExecutionEvent, ExecutionSignature, ExecutionStatus, ExecutionStepKind, SessionEvent,
    StepEvent, StepKind, StepResult,
};

use crate::step::Step;
use crate::step::execution::Execution;
use crate::task::TaskState;

pub struct ExecutionHub {
    execution_map: Mutex<HashMap<ExecutionSignature, Execution>>,
}

impl ExecutionHub {
    pub fn new() -> Self {
        Self {
            execution_map: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn run_execution_step(&self, state: &Arc<TaskState>, step: Step) {
        Step::send_step_event(state, &step.signature, StepEvent::Started);
        let request = match &step.signature.kind {
            StepKind::Execution(ExecutionStepKind::Invocation(request)) => request.clone(),
            _ => {
                let _ = Logger::warning("execution step kind not supported");
                Step::send_step_event(
                    state,
                    &step.signature,
                    StepEvent::Fail {
                        result: StepResult {
                            content: "execution step kind is not supported yet".to_owned(),
                        },
                    },
                );
                return;
            }
        };
        let execution = Execution::new(request.signature.clone(), step.signature.clone());
        self.execution_map
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(request.signature.clone(), execution);
        let _ = Logger::debug(format!(
            "evoking execution: tool {}",
            request.signature.name
        ));
        let _ = state.session_tx.send(SessionEvent::Execution(
            request.signature.clone(),
            ExecutionEvent::Evoke(request),
        ));
    }

    pub(crate) fn route_event(
        &self,
        state: &Arc<TaskState>,
        signature: ExecutionSignature,
        event: ExecutionEvent,
    ) {
        let completed = {
            let mut executions = self
                .execution_map
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let Some(execution) = executions.get_mut(&signature) else {
                return;
            };
            match event {
                ExecutionEvent::Update(update) => execution.push(update.seq, update.content),
                ExecutionEvent::Status(ExecutionStatus::Succeed(count)) => {
                    execution.finalize(count)
                }
                _ => return,
            }
        };
        if completed {
            self.on_complete(state, &signature);
        }
    }

    fn on_complete(&self, state: &Arc<TaskState>, signature: &ExecutionSignature) {
        let Some((step_signature, content)) = self
            .execution_map
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(signature)
            .map(|execution| (execution.step.clone(), execution.content()))
        else {
            return;
        };
        let _ = Logger::debug(format!("execution {} completed", signature.exe_id.0));
        Step::send_step_event(
            state,
            &step_signature,
            StepEvent::Complete {
                seq_count: 0,
                result: StepResult { content },
            },
        );
    }

    pub fn status(&self, signature: &ExecutionSignature) -> ExecutionStatus {
        // An untracked signature has not started yet, so it reports Started.
        self.execution_map
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(signature)
            .map(|execution| execution.status.clone())
            .unwrap_or(ExecutionStatus::Started)
    }
}
