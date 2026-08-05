use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use marix_common::external::*;
use marix_common::{
    Actor, ActorStartFuture, ActorStatus, Lifecycle,
    Runtime as RuntimeTrait,
};
use marix_protocol::{
    ContinuationRequest, ExecutionEvent, ExecutionRequest,
    ExecutionResult, ExecutionResultKind, ExecutionSignature,
    ExecutorEvent, InvocationEvent, InvocationResult,
    InvocationResultKind, InvocationSignature, RelayResult,
    RelayResultKind, RelaySignature, SessionEvent, StepEvent,
    TaskEvent, TaskLogging, ToolInputSchema,
};

use super::Invocation;
use crate::relay::Relay;
use crate::stage::{StageAssembler, StageResult, StageType};
use crate::task::TaskAccess;

pub struct InvocationRuntime {
    pub access: Arc<TaskAccess>,
    pub signature: InvocationSignature,
    pub input: ToolInputSchema,
    pub output: StdMutex<BTreeMap<usize, String>>,
    pub final_signal: StdMutex<Option<usize>>,
    pub execution: StdMutex<Option<ExecutionSignature>>,
    pub pending_stage: StdMutex<
        Option<(InvocationResultKind, RelaySignature)>,
    >,
    pub lifecycle: Lifecycle<InvocationEvent, InvocationResult>,
    summaries: StdMutex<Vec<String>>,
    overall_kind: StdMutex<Option<InvocationResultKind>>,
    pending_stage_cursor: StdMutex<Option<String>>,
    continuation_pending: StdMutex<bool>,
    stage_sequence: AtomicUsize,
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
            pending_stage: StdMutex::new(None),
            lifecycle: Lifecycle::new(),
            summaries: StdMutex::new(Vec::new()),
            overall_kind: StdMutex::new(None),
            pending_stage_cursor: StdMutex::new(None),
            continuation_pending: StdMutex::new(false),
            stage_sequence: AtomicUsize::new(0),
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
            let session_context = match self.access.session_context() {
                Ok(session_context) => session_context,
                Err(reason) => {
                    self.finish(InvocationResultKind::Failed, reason);
                    return None;
                }
            };
            if !Self::lock(&session_context)
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
                self.on_stage_update(signature, status);
            }
            InvocationEvent::Continuation {
                content,
                continuation_cursor,
            } => {
                self.on_continuation(
                    content,
                    continuation_cursor,
                );
            }
            InvocationEvent::ContinuationFailed { reason } => {
                if !self.take_continuation_pending() {
                    self.error(format!(
                        "invocation {} received continuation failure \
                         without a pending request",
                        &self.signature,
                    ));
                    return;
                }
                self.warning(format!(
                    "invocation {} continuation failed: {reason}",
                    &self.signature,
                ));
                let kind = self.update_overall_kind(
                    InvocationResultKind::Failed,
                );
                self.finish_summary(kind);
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
    fn lock<T>(
        mutex: &StdMutex<T>,
    ) -> std::sync::MutexGuard<'_, T> {
        mutex
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }

    fn create_execution(&self) -> Result<(), String> {
        if Self::lock(&self.pending_stage).is_some() {
            return Err(
                "cannot create execution while a stage relay is pending"
                    .to_owned(),
            );
        }
        let signature = ExecutionSignature::new(
            self.signature.clone(),
            self.signature.name.clone(),
        );
        {
            let mut execution = Self::lock(&self.execution);
            if let Some(active) = execution.as_ref() {
                return Err(format!(
                    "cannot create execution {}; execution {active} \
                     is still active",
                    &signature,
                ));
            }
            *execution = Some(signature.clone());
        }
        Self::lock(&self.output).clear();
        *Self::lock(&self.final_signal) = None;
        let result = self.send_executor_event(
            ExecutorEvent::ExecutionCreate(ExecutionRequest {
                signature: signature.clone(),
                input: self.input.clone(),
            }),
        );
        if result.is_err() {
            let mut execution = Self::lock(&self.execution);
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
        if Self::lock(&self.execution).as_ref() != Some(&execution) {
            self.error(format!(
                "invocation {} received processing update from \
                 unexpected execution {execution}",
                &self.signature,
            ));
            return;
        }
        Self::lock(&self.output).insert(seq, content);
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
            let mut current = Self::lock(&self.execution);
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
                        self.run_continuation_stage(
                            InvocationResultKind::Failed,
                            format!(
                                "invocation {} completed with missing \
                                 output chunks; expected {}",
                                &self.signature,
                                result.seq_count,
                            ),
                            None,
                        );
                        return;
                    };
                    *Self::lock(&self.final_signal) =
                        Some(result.seq_count);
                    self.run_continuation_stage(
                        InvocationResultKind::Succeed,
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
                    self.run_continuation_stage(
                        InvocationResultKind::Failed,
                        result.output,
                        result.continuation_cursor,
                    );
                }
            },
        }
    }

    fn run_continuation_stage(
        &self,
        kind: InvocationResultKind,
        output: String,
        continuation_cursor: Option<String>,
    ) {
        let kind = self.update_overall_kind(kind);
        let sequence =
            self.stage_sequence.fetch_add(1, Ordering::AcqRel) + 1;
        let signature = RelaySignature::new(
            self.signature.step.intent.clone(),
            format!("invocation-continue-{sequence}"),
        );
        let mut assembler =
            StageAssembler::new(StageType::InvocationContinue);
        assembler.inject(
            "tool",
            self.signature.name.clone(),
            None,
        );
        assembler.inject("output", output, None);
        if let Some(cursor) = continuation_cursor.as_ref() {
            assembler.inject(
                "continuation_cursor",
                cursor.clone(),
                None,
            );
        }
        for (index, summary) in
            Self::lock(&self.summaries).iter().enumerate()
        {
            assembler.inject(
                "previous_summary",
                summary.clone(),
                Some(format!("{index:08}")),
            );
        }
        let relay = match Relay::new(
            Arc::clone(&self.access),
            signature.clone(),
            assembler,
            Some(self.signature.clone()),
        ) {
            Ok(relay) => relay,
            Err(reason) => {
                self.warning(format!(
                    "invocation {} stage relay creation failed: \
                     {reason}",
                    &self.signature,
                ));
                self.finish_summary(kind);
                return;
            }
        };
        {
            let mut pending = Self::lock(&self.pending_stage);
            if let Some((_, active)) = pending.as_ref() {
                self.error(format!(
                    "invocation {} attempted to start stage relay {} \
                     while relay {active} is pending",
                    &self.signature,
                    relay.signature(),
                ));
                drop(pending);
                self.finish_summary(kind);
                return;
            }
            *pending = Some((kind.clone(), signature));
            *Self::lock(&self.pending_stage_cursor) =
                continuation_cursor;
        }
        if !self.access.insert(relay.clone()) {
            self.warning(format!(
                "invocation {} stage relay {} already exists",
                &self.signature,
                relay.signature(),
            ));
            *Self::lock(&self.pending_stage) = None;
            *Self::lock(&self.pending_stage_cursor) = None;
            self.finish_summary(kind);
            return;
        }
        relay.start();
    }

    fn on_stage_update(
        &self,
        signature: RelaySignature,
        status: ActorStatus<RelayResult>,
    ) {
        if matches!(self.status(), ActorStatus::Complete(_)) {
            self.error(format!(
                "invocation {} received stage relay {signature} \
                 update {status:?} after completion",
                &self.signature,
            ));
            return;
        }
        let ActorStatus::Complete(result) = status else {
            return;
        };
        let (kind, continuation_cursor) = {
            let mut pending = Self::lock(&self.pending_stage);
            let Some((kind, expected)) = pending.as_ref() else {
                self.error(format!(
                    "invocation {} received stage relay {signature} \
                     update with no pending state",
                    &self.signature,
                ));
                return;
            };
            if expected != &signature {
                self.error(format!(
                    "invocation {} received update from unexpected \
                     stage relay {signature}; expected {expected}",
                    &self.signature,
                ));
                return;
            }
            let kind = kind.clone();
            *pending = None;
            let continuation_cursor =
                Self::lock(&self.pending_stage_cursor).take();
            (kind, continuation_cursor)
        };
        if !matches!(result.kind, RelayResultKind::Succeed) {
            self.warning(format!(
                "invocation {} stage relay {signature} did not \
                 succeed: {}",
                &self.signature,
                result.output,
            ));
            self.finish_summary(kind);
            return;
        }
        let stage_result = match StageType::InvocationContinue
            .parse_result(&result.output)
        {
            Ok(result) => result,
            Err(reason) => {
                self.warning(format!(
                    "invocation {} stage relay {signature} returned \
                     invalid output: {reason}",
                    &self.signature,
                ));
                self.finish_summary(kind);
                return;
            }
        };
        match stage_result {
            StageResult::InvocationContinue(result) => {
                let Some(expected_cursor) = continuation_cursor else {
                    self.warning(format!(
                        "invocation {} stage relay {signature} requested \
                         continuation without an available cursor",
                        &self.signature,
                    ));
                    self.finish_summary(kind);
                    return;
                };
                if result.continuation_cursor != expected_cursor {
                    self.warning(format!(
                        "invocation {} stage relay {signature} returned \
                         continuation cursor `{}`; expected \
                         `{expected_cursor}`",
                        &self.signature,
                        result.continuation_cursor,
                    ));
                    self.finish_summary(kind);
                    return;
                }
                self.push_summary(result.summary);
                self.request_continuation(expected_cursor, kind);
            }
            StageResult::InvocationComplete(result) => {
                self.push_summary(result.summary);
                self.finish_summary(kind);
            }
            _ => {
                self.warning(format!(
                    "invocation {} received a non-invocation result \
                     from stage relay {signature}",
                    &self.signature,
                ));
                self.finish_summary(kind);
            }
        }
    }

    fn on_continuation(
        &self,
        content: String,
        continuation_cursor: Option<String>,
    ) {
        if matches!(self.status(), ActorStatus::Complete(_)) {
            self.error(format!(
                "invocation {} received continuation after completion",
                &self.signature,
            ));
            return;
        }
        if !self.take_continuation_pending() {
            self.error(format!(
                "invocation {} received continuation without a pending \
                 request",
                &self.signature,
            ));
            return;
        }
        let kind = Self::lock(&self.overall_kind)
            .clone()
            .unwrap_or(InvocationResultKind::Succeed);
        self.run_continuation_stage(
            kind,
            content,
            continuation_cursor,
        );
    }

    fn request_continuation(
        &self,
        continuation_cursor: String,
        kind: InvocationResultKind,
    ) {
        {
            let mut pending = Self::lock(&self.continuation_pending);
            if *pending {
                self.warning(format!(
                    "invocation {} attempted to request a continuation \
                     while another request is pending",
                    &self.signature,
                ));
                drop(pending);
                self.finish_summary(kind);
                return;
            }
            *pending = true;
        }
        let request = ContinuationRequest {
            invocation: self.signature.clone(),
            continuation_cursor,
        };
        if let Err(reason) = self.send_executor_event(
            ExecutorEvent::Continuation(request),
        ) {
            *Self::lock(&self.continuation_pending) = false;
            self.warning(format!(
                "invocation {} continuation request failed: {reason}",
                &self.signature,
            ));
            self.finish_summary(kind);
        }
    }

    fn take_continuation_pending(&self) -> bool {
        let mut pending = Self::lock(&self.continuation_pending);
        let was_pending = *pending;
        *pending = false;
        was_pending
    }

    fn push_summary(&self, summary: String) {
        let summary = summary.trim();
        if !summary.is_empty() {
            Self::lock(&self.summaries)
                .push(summary.to_owned());
        }
    }

    fn update_overall_kind(
        &self,
        kind: InvocationResultKind,
    ) -> InvocationResultKind {
        let mut overall = Self::lock(&self.overall_kind);
        let kind = if matches!(
            overall.as_ref(),
            Some(InvocationResultKind::Failed)
        ) || matches!(kind, InvocationResultKind::Failed)
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
        if output.len() != seq_count
            || (0..seq_count).any(|seq| !output.contains_key(&seq))
        {
            return None;
        }
        Some(
            (0..seq_count)
                .map(|seq| output[&seq].clone())
                .collect(),
        )
    }

    fn cancel(&self) {
        if matches!(self.status(), ActorStatus::Complete(_)) {
            return;
        }
        if let Some(signature) =
            Self::lock(&self.execution).clone()
            && let Err(reason) = self.send_executor_event(
                ExecutorEvent::Execution(
                    signature,
                    ExecutionEvent::Cancel,
                ),
            )
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

    fn finish(
        &self,
        kind: InvocationResultKind,
        output: String,
    ) {
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
                "executor event send failed: session stopped".to_owned()
            })
    }
}

#[allow(dead_code)]
fn assert_runtime_object_safe(
    runtime: &dyn RuntimeTrait<
        Base = Invocation,
        Prepared = (),
    >,
) {
    let _ = runtime.run();
}
