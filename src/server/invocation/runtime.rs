use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::external::*;
use marix_common::{
    ActorStartFuture, ActorStatus, Lifecycle, Runtime as RuntimeTrait,
};
use marix_protocol::{
    ExecutionEvent, ExecutionRequest, ExecutionResult,
    ExecutionResultKind, ExecutionSignature, ExecutorEvent,
    InvocationEvent, InvocationResult, InvocationResultKind,
    InvocationSignature, SessionEvent, StepEvent, TaskEvent,
    TaskLogging, ToolInputSchema,
};

use super::Invocation;
use crate::task::TaskAccess;

pub struct InvocationRuntime {
    pub access: Arc<TaskAccess>,
    pub signature: InvocationSignature,
    pub input: ToolInputSchema,
    pub output: StdMutex<BTreeMap<usize, String>>,
    pub execution: StdMutex<Option<ExecutionSignature>>,
    pub(super) pending_summarize: StdMutex<Option<Option<String>>>,
    pub lifecycle: Lifecycle<InvocationEvent, InvocationResult>,
    pub(super) summaries: StdMutex<Vec<String>>,
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
            execution: StdMutex::new(None),
            pending_summarize: StdMutex::new(None),
            lifecycle: Lifecycle::new(),
            summaries: StdMutex::new(Vec::new()),
        }
    }
}

impl RuntimeTrait for InvocationRuntime {
    type Base = Invocation;
    type Prepared = ();

    fn signature(&self) -> &InvocationSignature {
        &self.signature
    }

    fn lifecycle(
        &self,
    ) -> &Lifecycle<InvocationEvent, InvocationResult> {
        &self.lifecycle
    }

    fn on_start(&self) -> ActorStartFuture<'_, Self::Prepared> {
        Box::pin(async move {
            let session_context = match self.access.session_context()
            {
                Ok(session_context) => session_context,
                Err(reason) => {
                    self.finish(InvocationResultKind::Failed, reason);
                    return None;
                }
            };
            if !session_context
                .lock()
                .unwrap()
                .is_valid_tool(&self.signature.name)
            {
                self.finish(
                    InvocationResultKind::Failed,
                    format!(
                        "tool '{}' is not available",
                        self.signature.name,
                    ),
                );
                return None;
            }
            if let Err(error) =
                serde_json::from_str::<serde_json::Value>(&self.input)
            {
                self.finish(
                    InvocationResultKind::Failed,
                    format!(
                        "arguments for tool '{}' are invalid JSON: \
                         {error}",
                        self.signature.name,
                    ),
                );
                return None;
            }
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
            InvocationEvent::SummarizeUpdate(signature, status) => {
                self.on_summary_update(signature, status);
            }
            InvocationEvent::Continuation {
                content,
                continuation_cursor,
            } => {
                self.on_continuation(content, continuation_cursor);
            }
            InvocationEvent::ContinuationFailed { reason } => {
                self.on_continuation_failed(reason);
            }
            InvocationEvent::Cancel => self.cancel(),
        }
    }

    fn on_finish(&self, result: InvocationResult) {
        self.send_step_update(ActorStatus::Complete(result));
    }
}

// -- Private -- //

impl InvocationRuntime {
    fn create_execution(&self) -> Result<(), String> {
        let signature = ExecutionSignature::new(
            self.signature.clone(),
            self.signature.name.clone(),
        );
        {
            let mut execution = self.execution.lock().unwrap();
            if let Some(active) = execution.as_ref() {
                return Err(format!(
                    "cannot create execution {}; execution {active} \
                     is still active",
                    &signature,
                ));
            }
            *execution = Some(signature.clone());
        }
        self.output.lock().unwrap().clear();
        let result = self.send_executor_event(
            ExecutorEvent::ExecutionCreate(ExecutionRequest {
                signature: signature.clone(),
                input: self.input.clone(),
            }),
        );
        if result.is_err() {
            let mut execution = self.execution.lock().unwrap();
            if execution.as_ref() == Some(&signature) {
                *execution = None;
            }
        }
        result
    }

    fn on_processing(
        &self,
        execution: ExecutionSignature,
        seq: usize,
        content: String,
    ) {
        if matches!(self.status(), ActorStatus::Complete(_)) {
            self.error(format!(
                "invocation {} received processing update from \
                 execution {execution} after completion",
                &self.signature,
            ));
            return;
        }
        if self.execution.lock().unwrap().as_ref() != Some(&execution) {
            self.error(format!(
                "invocation {} received processing update from \
                 unexpected execution {execution}",
                &self.signature,
            ));
            return;
        }
        self.output.lock().unwrap().insert(seq, content);
    }

    fn on_update(
        &self,
        execution: ExecutionSignature,
        status: ActorStatus<ExecutionResult>,
    ) {
        if matches!(self.status(), ActorStatus::Complete(_)) {
            self.error(format!(
                "invocation {} received execution {execution} update \
                 {status:?} after completion",
                &self.signature,
            ));
            return;
        }
        let complete = matches!(&status, ActorStatus::Complete(_));
        {
            let mut current = self.execution.lock().unwrap();
            if current.as_ref() != Some(&execution) {
                self.error(format!(
                    "invocation {} received update from unexpected \
                     execution {execution}: {status:?}",
                    &self.signature,
                ));
                return;
            }
            if complete {
                *current = None;
            }
        }
        match status {
            ActorStatus::Created | ActorStatus::Running => {}
            ActorStatus::Complete(result) => match result.kind {
                ExecutionResultKind::Succeed => {
                    let Some(output) =
                        self.complete_output(result.seq_count)
                    else {
                        self.finish(
                            InvocationResultKind::Failed,
                            format!(
                                "invocation {} completed with missing \
                                 output chunks; expected {}",
                                &self.signature, result.seq_count,
                            ),
                        );
                        return;
                    };
                    self.request_summary_decision(
                        output,
                        result.continuation_cursor,
                    );
                }
                ExecutionResultKind::Canceled => {
                    self.finish(
                        InvocationResultKind::Canceled,
                        result.output,
                    );
                }
                ExecutionResultKind::Failed => {
                    self.request_summary_decision(
                        result.output,
                        result.continuation_cursor,
                    );
                }
            },
        }
    }

    fn complete_output(&self, seq_count: usize) -> Option<String> {
        let output = self.output.lock().unwrap();
        if output.len() != seq_count
            || (0..seq_count).any(|seq| !output.contains_key(&seq))
        {
            return None;
        }
        Some((0..seq_count).map(|seq| output[&seq].clone()).collect())
    }

    fn cancel(&self) {
        if matches!(self.status(), ActorStatus::Complete(_)) {
            return;
        }
        if let Some(signature) = self.execution.lock().unwrap().clone()
            && let Err(reason) =
                self.send_executor_event(ExecutorEvent::Execution(
                    signature,
                    ExecutionEvent::Cancel,
                ))
        {
            self.warning(format!(
                "invocation {} execution cancel failed: {reason}",
                &self.signature,
            ));
        }
        self.finish(
            InvocationResultKind::Canceled,
            "invocation canceled".to_owned(),
        );
    }

    fn finish(&self, kind: InvocationResultKind, output: String) {
        let seq_count = self.output.lock().unwrap().len();
        RuntimeTrait::finish(
            self,
            InvocationResult {
                kind,
                output,
                seq_count,
            },
        );
    }

    fn send_step_update(
        &self,
        status: ActorStatus<InvocationResult>,
    ) {
        let step = self.signature.step.clone();
        let event = SessionEvent::Task(
            step.intent.task.clone(),
            TaskEvent::Step(
                step,
                StepEvent::Update(self.signature.clone(), status),
            ),
        );
        if self.access.session_tx.send(event).is_err() {
            self.warning(format!(
                "invocation {} event send failed: session stopped",
                &self.signature,
            ));
        }
    }

    fn send_executor_event(
        &self,
        event: ExecutorEvent,
    ) -> Result<(), String> {
        self.access
            .session_tx
            .send(SessionEvent::Executor(event))
            .map_err(|_| {
                "executor event send failed: session stopped"
                    .to_owned()
            })
    }
}

#[allow(dead_code)]
fn assert_runtime_object_safe(
    runtime: &dyn RuntimeTrait<Base = Invocation, Prepared = ()>,
) {
    let _ = runtime.run();
}
