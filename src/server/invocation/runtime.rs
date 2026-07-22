use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::external::*;
use marix_common::{
    Actor, ActorStartFuture, ActorStatus, Lifecycle, Logger, Runtime as RuntimeTrait,
};
use marix_protocol::{
    ExecutionEvent, ExecutionRequest, ExecutionResult, ExecutionResultKind, ExecutionSignature,
    ExecutorEvent, InvocationEvent, InvocationResult, InvocationResultKind, InvocationSignature,
    RelayKind, RelayRequest, RelayResult, RelayResultKind, RelaySignature, SessionEvent, StepEvent,
    TaskEvent, ToolInputSchema,
};

use super::Invocation;
use crate::relay::Relay;
use crate::task::TaskAccess;

pub struct InvocationRuntime {
    pub access: Arc<TaskAccess>,
    pub signature: InvocationSignature,
    pub input: ToolInputSchema,
    pub output: StdMutex<BTreeMap<usize, String>>,
    pub final_signal: StdMutex<Option<usize>>,
    pub execution: StdMutex<Option<ExecutionSignature>>,
    pub pending_summarize: StdMutex<Option<(InvocationResultKind, String)>>,
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
            pending_summarize: StdMutex::new(None),
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
            let session_context = match self.access.session_context() {
                Ok(session_context) => session_context,
                Err(reason) => {
                    self.finish(InvocationResultKind::Failed, reason);
                    return None;
                }
            };
            let is_valid_tool = session_context
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .is_valid_tool(&self.signature.name);
            if !is_valid_tool {
                self.finish(
                    InvocationResultKind::Failed,
                    format!("tool '{}' is not available", self.signature.name),
                );
                return None;
            }
            if let Err(error) = serde_json::from_str::<serde_json::Value>(&self.input) {
                self.finish(
                    InvocationResultKind::Failed,
                    format!(
                        "arguments for tool '{}' are invalid JSON: {error}",
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
                self.on_summarize_update(signature, status);
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
        if matches!(self.status(), ActorStatus::Complete(_)) {
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

    fn on_update(&self, execution: ExecutionSignature, status: ActorStatus<ExecutionResult>) {
        if matches!(self.status(), ActorStatus::Complete(_)) {
            Logger::error(format!(
                "invocation {} received execution {execution} update \
                 {status:?} after completion",
                &self.signature,
            ));
            return;
        }
        match status {
            ActorStatus::Created | ActorStatus::Running => {}
            ActorStatus::Complete(result) => match result.kind {
                ExecutionResultKind::Succeed => {
                    let Some(output) = self.complete_output(result.seq_count) else {
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
                    *self
                        .final_signal
                        .lock()
                        .unwrap_or_else(|error| error.into_inner()) = Some(result.seq_count);
                    self.summarize_and_finish(InvocationResultKind::Succeed, output);
                }
                ExecutionResultKind::Canceled => {
                    self.finish(InvocationResultKind::Canceled, result.output);
                }
                ExecutionResultKind::Failed => {
                    self.summarize_and_finish(InvocationResultKind::Failed, result.output);
                }
            },
        }
    }

    fn summarize_and_finish(&self, kind: InvocationResultKind, output: String) {
        let relay_signature = RelaySignature::new(
            self.signature.step.intent.clone(),
            "tool-call-summarize".to_owned(),
        );
        let request = RelayRequest {
            signature: relay_signature,
            kind: RelayKind::ToolCallSummarize {
                invocation: self.signature.clone(),
                tool: self.signature.name.clone(),
                output: output.clone(),
            },
        };
        let relay = match Relay::new(Arc::clone(&self.access), request) {
            Ok(relay) => relay,
            Err(reason) => {
                Logger::warning(format!(
                    "invocation {} summarize relay creation failed: {reason}; keeping original output",
                    &self.signature,
                ));
                self.finish(kind, output);
                return;
            }
        };
        *self
            .pending_summarize
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some((kind.clone(), output.clone()));
        if !self.access.insert(relay.clone()) {
            Logger::warning(format!(
                "invocation {} summarize relay {} already exists; keeping original output",
                &self.signature,
                relay.signature(),
            ));
            *self
                .pending_summarize
                .lock()
                .unwrap_or_else(|error| error.into_inner()) = None;
            self.finish(kind, output);
            return;
        }
        relay.start();
    }

    fn on_summarize_update(&self, signature: RelaySignature, status: ActorStatus<RelayResult>) {
        if matches!(self.status(), ActorStatus::Complete(_)) {
            Logger::error(format!(
                "invocation {} received summarize relay {signature} update {status:?} after completion",
                &self.signature,
            ));
            return;
        }
        let ActorStatus::Complete(result) = status else {
            return;
        };
        if signature.name != "tool-call-summarize" {
            Logger::error(format!(
                "invocation {} received update from unexpected relay name `{}`",
                &self.signature, signature.name,
            ));
            return;
        }
        let Some((kind, original_output)) = self
            .pending_summarize
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
        else {
            Logger::error(format!(
                "invocation {} received summarize relay {signature} update with no pending state",
                &self.signature,
            ));
            return;
        };
        let final_output = match result.kind {
            RelayResultKind::Succeed => result.output,
            RelayResultKind::Failed | RelayResultKind::Canceled => {
                Logger::warning(format!(
                    "invocation {} summarize relay {signature} did not succeed: {}; \
                     keeping original output",
                    &self.signature, result.output,
                ));
                original_output
            }
        };
        self.finish(kind, final_output);
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
        if matches!(self.status(), ActorStatus::Complete(_)) {
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

    fn send_step_update(&self, status: ActorStatus<InvocationResult>) {
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
