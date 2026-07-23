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
    TaskEvent, ToolInputSchema, WorkflowCallSummary, WorkflowContinuation, WorkflowTool,
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
    summaries: StdMutex<Vec<String>>,
    overall_kind: StdMutex<Option<InvocationResultKind>>,
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
            summaries: StdMutex::new(Vec::new()),
            overall_kind: StdMutex::new(None),
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
            let is_valid_tool = self.signature.name == WorkflowContinuation::NAME
                || Self::lock(&session_context).is_valid_tool(&self.signature.name);
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
    fn lock<T>(mutex: &StdMutex<T>) -> std::sync::MutexGuard<'_, T> {
        mutex.lock().unwrap_or_else(|error| error.into_inner())
    }

    fn create_execution(&self) -> Result<(), String> {
        self.request_execution(self.signature.name.clone(), self.input.clone())
    }

    fn request_execution(&self, name: String, input: ToolInputSchema) -> Result<(), String> {
        if Self::lock(&self.pending_summarize).is_some() {
            return Err("cannot create execution while a summarize relay is pending".to_owned());
        }
        let signature = ExecutionSignature::new(self.signature.clone(), name);
        {
            let mut execution = Self::lock(&self.execution);
            if let Some(active) = execution.as_ref() {
                return Err(format!(
                    "cannot create execution {}; execution {active} is still active",
                    &signature
                ));
            }
            *execution = Some(signature.clone());
        }
        Self::lock(&self.output).clear();
        *Self::lock(&self.final_signal) = None;
        let result = self.send_executor_event(ExecutorEvent::ExecutionCreate(ExecutionRequest {
            signature: signature.clone(),
            input,
        }));
        if result.is_err() {
            let mut execution = Self::lock(&self.execution);
            if execution.as_ref() == Some(&signature) {
                *execution = None;
            }
        }
        result
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
        let current = Self::lock(&self.execution).clone();
        if current.as_ref() != Some(&execution) {
            Logger::error(format!(
                "invocation {} received processing update from unexpected \
                 execution {execution}",
                &self.signature,
            ));
            return;
        }
        Self::lock(&self.output).insert(seq, content);
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
        let complete = matches!(&status, ActorStatus::Complete(_));
        {
            let mut current = Self::lock(&self.execution);
            if current.as_ref() != Some(&execution) {
                Logger::error(format!(
                    "invocation {} received update from unexpected execution \
                     {execution}: {status:?}",
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
                    let Some(output) = self.complete_output(result.seq_count) else {
                        self.summarize_and_finish(
                            InvocationResultKind::Failed,
                            format!(
                                "invocation {} completed with missing \
                                 output chunks; expected {}",
                                &self.signature, result.seq_count,
                            ),
                            None,
                        );
                        return;
                    };
                    *Self::lock(&self.final_signal) = Some(result.seq_count);
                    self.summarize_and_finish(
                        InvocationResultKind::Succeed,
                        output,
                        result.continuation_cursor,
                    );
                }
                ExecutionResultKind::Canceled => {
                    self.finish(InvocationResultKind::Canceled, result.output);
                }
                ExecutionResultKind::Failed => {
                    self.summarize_and_finish(
                        InvocationResultKind::Failed,
                        result.output,
                        result.continuation_cursor,
                    );
                }
            },
        }
    }

    fn summarize_and_finish(
        &self,
        kind: InvocationResultKind,
        output: String,
        continuation_cursor: Option<String>,
    ) {
        let kind = self.update_overall_kind(kind);
        let relay_signature = RelaySignature::new(
            self.signature.step.intent.clone(),
            "tool-call-summarize".to_owned(),
        );
        let request = RelayRequest {
            signature: relay_signature,
            kind: RelayKind::ToolCallSummarize {
                invocation: self.signature.clone(),
                tool: self.signature.name.clone(),
                output,
                continuation_cursor,
            },
        };
        let relay = match Relay::new(Arc::clone(&self.access), request) {
            Ok(relay) => relay,
            Err(reason) => {
                Logger::warning(format!(
                    "invocation {} summarize relay creation failed: {reason}",
                    &self.signature,
                ));
                self.finish_summary(kind);
                return;
            }
        };
        {
            let mut pending = Self::lock(&self.pending_summarize);
            if pending.is_some() {
                Logger::error(format!(
                    "invocation {} attempted to start summarize relay {} \
                     while another summarize relay is pending",
                    &self.signature,
                    relay.signature(),
                ));
                drop(pending);
                self.finish_summary(kind);
                return;
            }
            *pending = Some((kind.clone(), relay.signature().to_string()));
        }
        if !self.access.insert(relay.clone()) {
            Logger::warning(format!(
                "invocation {} summarize relay {} already exists",
                &self.signature,
                relay.signature(),
            ));
            *Self::lock(&self.pending_summarize) = None;
            self.finish_summary(kind);
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
        let kind = {
            let mut pending = Self::lock(&self.pending_summarize);
            let Some((kind, expected_signature)) = pending.as_ref() else {
                Logger::error(format!(
                    "invocation {} received summarize relay {signature} \
                     update with no pending state",
                    &self.signature,
                ));
                return;
            };
            if expected_signature != &signature.to_string() {
                Logger::error(format!(
                    "invocation {} received update from unexpected summarize \
                     relay {signature}",
                    &self.signature,
                ));
                return;
            }
            let kind = kind.clone();
            *pending = None;
            kind
        };
        match result.kind {
            RelayResultKind::Succeed => {
                let tool = match WorkflowCallSummary::parse(&result.output) {
                    Ok(tool) => tool,
                    Err(error) => {
                        Logger::warning(format!(
                            "invocation {} summarize relay {signature} \
                             returned invalid output: {error}",
                            &self.signature,
                        ));
                        self.finish_summary(kind);
                        return;
                    }
                };
                let summary = tool.summary.trim();
                if !summary.is_empty() {
                    Self::lock(&self.summaries).push(summary.to_owned());
                }
                if let Some(cursor) = tool.continuation_cursor {
                    self.request_continuation(cursor, kind);
                } else {
                    self.finish_summary(kind);
                }
            }
            RelayResultKind::Failed | RelayResultKind::Canceled => {
                Logger::warning(format!(
                    "invocation {} summarize relay {signature} did not \
                     succeed: {}",
                    &self.signature, result.output,
                ));
                self.finish_summary(kind);
            }
        }
    }

    fn request_continuation(&self, cursor: String, kind: InvocationResultKind) {
        let input = match serde_json::to_string(&WorkflowContinuation {
            continuation_cursor: cursor,
        }) {
            Ok(input) => input,
            Err(error) => {
                Logger::warning(format!(
                    "invocation {} continuation input serialization failed: \
                     {error}",
                    &self.signature,
                ));
                self.finish_summary(kind);
                return;
            }
        };
        if let Err(reason) = self.request_execution(WorkflowContinuation::NAME.to_owned(), input) {
            Logger::warning(format!(
                "invocation {} continuation execution create failed: {reason}",
                &self.signature,
            ));
            self.finish_summary(kind);
        }
    }

    fn update_overall_kind(&self, kind: InvocationResultKind) -> InvocationResultKind {
        let mut overall = Self::lock(&self.overall_kind);
        let kind = if matches!(overall.as_ref(), Some(InvocationResultKind::Failed))
            || matches!(kind, InvocationResultKind::Failed)
        {
            InvocationResultKind::Failed
        } else {
            InvocationResultKind::Succeed
        };
        *overall = Some(kind.clone());
        kind
    }

    fn finish_summary(&self, kind: InvocationResultKind) {
        let output = {
            let summaries = Self::lock(&self.summaries);
            if summaries.is_empty() {
                "No Summary".to_owned()
            } else {
                summaries.join("\n")
            }
        };
        self.finish(kind, output);
    }

    fn complete_output(&self, seq_count: usize) -> Option<String> {
        let output = Self::lock(&self.output);
        if output.len() != seq_count || (0..seq_count).any(|seq| !output.contains_key(&seq)) {
            return None;
        }
        Some((0..seq_count).map(|seq| output[&seq].clone()).collect())
    }

    fn cancel(&self) {
        if matches!(self.status(), ActorStatus::Complete(_)) {
            return;
        }
        let execution = Self::lock(&self.execution).clone();
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
        let seq_count = Self::lock(&self.output).len();
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
