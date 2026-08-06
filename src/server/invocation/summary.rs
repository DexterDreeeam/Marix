use std::sync::Arc;

use marix_common::external::*;
use marix_common::{
    Actor, ActorStatus, Runtime as RuntimeTrait,
};
use marix_protocol::{
    ContinuationRequest, ExecutorEvent, InvocationResult,
    InvocationResultKind, RelayResult, RelayResultKind,
    RelaySignature, SessionEvent, TaskLogging,
};

use super::runtime::InvocationRuntime;
use crate::prompt::PromptProfile;
use crate::relay::{Relay, RelayOwner};
use crate::stage::StageAssembler;

impl InvocationRuntime {
    pub(super) fn request_summary_decision(
        &self,
        output: String,
        continuation_cursor: Option<String>,
    ) {
        let signature = RelaySignature::new(
            self.signature.step.intent.clone(),
            "invocation-summary".to_owned(),
        );
        let summaries = self
            .summaries
            .lock()
            .unwrap()
            .clone();
        let mut assembler = StageAssembler::new(
            "InvocationSummary".to_owned(),
            PromptProfile::StageJson,
        );
        assembler.inject(
            "tool",
            self.signature.name.clone(),
            None,
        );
        assembler.inject("output", output, None);
        if let Some(cursor) = continuation_cursor.as_deref() {
            assembler.inject(
                "continuation_cursor",
                cursor.to_owned(),
                None,
            );
        }
        for (index, summary) in summaries.into_iter().enumerate() {
            assembler.inject(
                "previous_summary",
                summary,
                Some(format!("{index:08}")),
            );
        }
        let request = match assembler.assemble(
            &self.access,
            signature.clone(),
        ) {
            Ok(request) => request,
            Err(reason) => {
                let reason = format!(
                    "invocation {} summary assembly failed: {reason}",
                    &self.signature,
                );
                self.warning(reason.clone());
                self.finish_summary_failed(reason);
                return;
            }
        };
        let relay = match Relay::new(
            Arc::clone(&self.access),
            request,
            RelayOwner::Invocation(self.signature.clone()),
        ) {
            Ok(relay) => relay,
            Err(reason) => {
                let reason = format!(
                    "invocation {} summary relay creation failed: \
                     {reason}",
                    &self.signature,
                );
                self.warning(reason.clone());
                self.finish_summary_failed(reason);
                return;
            }
        };
        *self
            .pending_summarize
            .lock()
            .unwrap() = Some(continuation_cursor);
        if !self.access.insert(relay.clone()) {
            let reason = format!(
                "invocation {} summary relay {} already exists",
                &self.signature,
                relay.signature(),
            );
            self.warning(reason.clone());
            *self
                .pending_summarize
                .lock()
                .unwrap() = None;
            self.finish_summary_failed(reason);
            return;
        }
        relay.start();
    }

    pub(super) fn on_summary_update(
        &self,
        signature: RelaySignature,
        status: ActorStatus<RelayResult>,
    ) {
        if matches!(self.status(), ActorStatus::Complete(_)) {
            self.error(format!(
                "invocation {} received summary relay {signature} \
                 update {status:?} after completion",
                &self.signature,
            ));
            return;
        }
        let ActorStatus::Complete(result) = status else {
            return;
        };
        let continuation_cursor = {
            let mut pending = self
                .pending_summarize
                .lock()
                .unwrap();
            let Some(pending) = pending.take() else {
                let reason = format!(
                    "invocation {} received summary relay {signature} \
                     update with no pending state",
                    &self.signature,
                );
                self.error(reason.clone());
                self.finish_summary_failed(reason);
                return;
            };
            pending
        };
        if !matches!(result.kind, RelayResultKind::Succeed) {
            let reason = format!(
                "invocation {} summary relay {signature} did not \
                 succeed: {}",
                &self.signature, result.output,
            );
            self.warning(reason.clone());
            self.finish_summary_failed(reason);
            return;
        }
        let decision = match InvocationDecision::parse(&result.output) {
            Ok(decision) => decision,
            Err(reason) => {
                let reason = format!(
                    "invocation {} summary relay {signature} returned \
                     invalid output: {reason}",
                    &self.signature,
                );
                self.warning(reason.clone());
                self.finish_summary_failed(reason);
                return;
            }
        };
        if let Err(reason) = decision.apply(self, continuation_cursor) {
            let reason = format!(
                "invocation {} summary relay {signature} returned \
                 invalid output: {reason}",
                &self.signature,
            );
            self.warning(reason.clone());
            self.finish_summary_failed(reason);
        }
    }

    pub(super) fn on_continuation(
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
        self.request_summary_decision(content, continuation_cursor);
    }

    pub(super) fn on_continuation_failed(&self, reason: String) {
        let reason = format!(
            "invocation {} continuation failed: {reason}",
            &self.signature,
        );
        self.warning(reason.clone());
        self.finish_summary_failed(reason);
    }
}

// -- Private -- //

#[derive(Debug, Deserialize)]
#[serde(
    tag = "result",
    content = "payload",
    deny_unknown_fields
)]
enum InvocationDecision {
    #[serde(rename = "invocation_continue")]
    Continue {
        summary: String,
        continuation_cursor: String,
    },
    #[serde(rename = "invocation_complete")]
    Complete {
        summary: String,
    },
}

impl InvocationDecision {
    fn parse(output: &str) -> Result<Self, String> {
        serde_json::from_str(output).map_err(|error| {
            format!(
                "invocation summary decision is invalid JSON: {error}"
            )
        })
    }

    fn apply(
        self,
        runtime: &InvocationRuntime,
        continuation_cursor: Option<String>,
    ) -> Result<(), String> {
        match self {
            InvocationDecision::Continue {
                summary,
                continuation_cursor: returned_cursor,
            } => {
                let Some(continuation_cursor) = continuation_cursor else {
                    return Err(format!(
                        "requested continuation without an available \
                         cursor for invocation {}",
                        &runtime.signature,
                    ));
                };
                if returned_cursor != continuation_cursor {
                    return Err(format!(
                        "returned continuation cursor \
                         `{returned_cursor}`; expected \
                         `{continuation_cursor}` for invocation {}",
                        &runtime.signature,
                    ));
                }
                runtime.push_summary(summary);
                runtime.request_continuation(continuation_cursor);
            }
            InvocationDecision::Complete { summary } => {
                runtime.push_summary(summary);
                runtime.finish_summary();
            }
        }
        Ok(())
    }
}

impl InvocationRuntime {
    fn request_continuation(&self, continuation_cursor: String) {
        let request = ContinuationRequest {
            invocation: self.signature.clone(),
            continuation_cursor,
        };
        let result = self
            .access
            .session_tx
            .send(SessionEvent::Executor(
                ExecutorEvent::Continuation(request),
            ))
            .map_err(|_| {
                "executor event send failed: session stopped".to_owned()
            });
        if let Err(reason) = result {
            let reason = format!(
                "invocation {} continuation request failed: {reason}",
                &self.signature,
            );
            self.warning(reason.clone());
            self.finish_summary_failed(reason);
        }
    }

    fn push_summary(&self, summary: String) {
        let summary = summary.trim();
        if !summary.is_empty() {
            self.summaries
                .lock()
                .unwrap()
                .push(summary.to_owned());
        }
    }

    fn finish_summary(&self) {
        let output = self.summary_output();
        let seq_count = self
            .output
            .lock()
            .unwrap()
            .len();
        RuntimeTrait::finish(
            self,
            InvocationResult {
                kind: InvocationResultKind::Succeed,
                output,
                seq_count,
            },
        );
    }

    fn finish_summary_failed(&self, reason: String) {
        let summaries = self.summaries.lock().unwrap();
        let output = if summaries.is_empty() {
            format!("Invocation failed: {reason}")
        } else {
            format!(
                "{}\nInvocation failed: {reason}",
                summaries.join("\n"),
            )
        };
        drop(summaries);
        let seq_count = self.output.lock().unwrap().len();
        RuntimeTrait::finish(
            self,
            InvocationResult {
                kind: InvocationResultKind::Failed,
                output,
                seq_count,
            },
        );
    }

    fn summary_output(&self) -> String {
        let summaries = self.summaries.lock().unwrap();
        if summaries.is_empty() {
            "No Summary".to_owned()
        } else {
            summaries.join("\n")
        }
    }
}
