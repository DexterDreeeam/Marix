use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::{ActorStartFuture, ActorStatus, Lifecycle, Logger, Runtime as RuntimeTrait};
use marix_protocol::{
    ExecutionEvent, ExecutionRequest, ExecutionSignature, ExecutionStatus, ExecutorEvent,
    InvocationEvent, InvocationResult, InvocationResultKind, InvocationSignature, SessionEvent,
    StepEvent, TaskEvent, ToolInputSchema,
};

use super::Invocation;
use crate::task::TaskAccess;

pub struct InvocationRuntime {
    pub access: Arc<TaskAccess>,
    pub signature: InvocationSignature,
    pub input: ToolInputSchema,
    pub output: StdMutex<BTreeMap<usize, String>>,
    pub final_signal: StdMutex<Option<usize>>,
    pub execution: StdMutex<Option<ExecutionSignature>>,
    pub lifecycle: Lifecycle<InvocationEvent, InvocationResult>,
}

impl InvocationRuntime {
    pub(crate) fn new(
        access: Arc<TaskAccess>,
        signature: InvocationSignature,
        input: ToolInputSchema,
    ) -> Self {
        Self {
            access,
            signature,
            input,
            output: StdMutex::new(BTreeMap::new()),
            final_signal: StdMutex::new(None),
            execution: StdMutex::new(None),
            lifecycle: Lifecycle::new(),
        }
    }
}

impl RuntimeTrait for InvocationRuntime {
    type Base = Invocation;
    type Prepared = ();

    fn signature(&self) -> &InvocationSignature {
        &self.signature
    }

    fn lifecycle(&self) -> &Lifecycle<InvocationEvent, InvocationResult> {
        &self.lifecycle
    }

    fn on_start(&self) -> ActorStartFuture<'_, Self::Prepared> {
        Box::pin(async move {
            if let Err(reason) = self.create_execution() {
                self.finish(InvocationResultKind::Failed, reason);
                return None;
            }
            Some(())
        })
    }

    fn dispatch(&self, event: InvocationEvent) {
        match event {
            InvocationEvent::Update(execution, status) => {
                self.on_update(execution, status);
            }
            InvocationEvent::Processing {
                execution,
                seq,
                content,
            } => {
                self.on_processing(execution, seq, content);
            }
            InvocationEvent::Cancel => self.cancel(),
        }
    }

    fn on_finish(&self) {
        self.send_step_update(ActorStatus::Complete);
    }
}

// -- Private -- //

impl InvocationRuntime {
    fn create_execution(&self) -> Result<(), String> {
        let signature =
            ExecutionSignature::new(self.signature.clone(), self.signature.name.clone());
        *self
            .execution
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(signature.clone());
        self.send_executor_event(ExecutorEvent::ExecutionCreate(ExecutionRequest {
            signature,
            input: self.input.clone(),
        }))
    }

    fn on_processing(&self, execution: ExecutionSignature, seq: usize, content: String) {
        if self.status().is_terminal() {
            Logger::error(format!(
                "invocation {} received processing update from \
                 execution {execution} after completion",
                &self.signature,
            ));
            return;
        }
        self.output
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(seq, content);
    }

    fn on_update(&self, execution: ExecutionSignature, status: ExecutionStatus) {
        if self.status().is_terminal() {
            Logger::error(format!(
                "invocation {} received execution {execution} update \
                 {status:?} after completion",
                &self.signature,
            ));
            return;
        }
        match status {
            ExecutionStatus::Created | ExecutionStatus::Started => {}
            ExecutionStatus::Succeed { seq_count } => {
                let Some(output) = self.complete_output(seq_count) else {
                    self.finish(
                        InvocationResultKind::Failed,
                        format!(
                            "invocation {} completed with missing \
                             output chunks; expected {seq_count}",
                            &self.signature,
                        ),
                    );
                    return;
                };
                *self
                    .final_signal
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = Some(seq_count);
                self.finish(InvocationResultKind::Succeed, output);
            }
            ExecutionStatus::Canceled => {
                self.finish(
                    InvocationResultKind::Canceled,
                    format!("execution {execution} canceled"),
                );
            }
            ExecutionStatus::Failed => {
                self.finish(
                    InvocationResultKind::Failed,
                    format!("execution {execution} failed"),
                );
            }
        }
    }

    fn complete_output(&self, seq_count: usize) -> Option<String> {
        let output = self
            .output
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if output.len() != seq_count || (0..seq_count).any(|seq| !output.contains_key(&seq)) {
            return None;
        }
        Some(
            (0..seq_count)
                .filter_map(|seq| output.get(&seq))
                .cloned()
                .collect(),
        )
    }

    fn cancel(&self) {
        if self.status().is_terminal() {
            return;
        }
        let execution = self
            .execution
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        if let Some(signature) = execution {
            if let Err(reason) = self
                .send_executor_event(ExecutorEvent::Execution(signature, ExecutionEvent::Cancel))
            {
                Logger::warning(format!(
                    "invocation {} execution cancel failed: {reason}",
                    &self.signature,
                ));
            }
        }
        self.finish(
            InvocationResultKind::Canceled,
            "invocation canceled".to_owned(),
        );
    }

    fn finish(&self, kind: InvocationResultKind, output: String) {
        let seq_count = self
            .output
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .len();
        RuntimeTrait::finish(
            self,
            InvocationResult {
                kind,
                output,
                seq_count,
            },
        );
    }

    fn send_step_update(&self, status: ActorStatus) {
        let step = self.signature.step.clone();
        let event = SessionEvent::Task(
            step.intent.task.clone(),
            TaskEvent::Step(step, StepEvent::Update(self.signature.clone(), status)),
        );
        if self.access.session_tx.send(event).is_err() {
            Logger::warning(format!(
                "invocation {} event send failed: session stopped",
                &self.signature,
            ));
        }
    }

    fn send_executor_event(&self, event: ExecutorEvent) -> Result<(), String> {
        self.access
            .session_tx
            .send(SessionEvent::Executor(event))
            .map_err(|_| "executor event send failed: session stopped".to_owned())
    }
}

#[allow(dead_code)]
fn assert_runtime_object_safe(runtime: &dyn RuntimeTrait<Base = Invocation, Prepared = ()>) {
    let _ = runtime.run();
}
