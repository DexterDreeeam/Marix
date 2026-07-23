use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::external::*;
use marix_common::{
    ActorCloseReceiver, ActorEventReceiver, ActorFuture, ActorStartFuture, ActorStatus, Config,
    Lifecycle, Logger, Runtime as RuntimeTrait,
};
use marix_protocol::{
    IntentEvent, InvocationEvent, RelayEvent, RelayKind, RelayRequest, RelayResult,
    RelayResultKind, RelaySignature, SessionEvent, StepDraft, TaskEvent, WorkflowCallSummary,
    WorkflowTool,
};

use super::Relay;
use crate::model::{DeepseekBackend, GlmBackend, ModelBackend, ModelResponse, ModelResponseStream};
use crate::task::TaskAccess;

pub struct RelayRuntime {
    pub access: Arc<TaskAccess>,
    pub signature: RelaySignature,
    pub kind: RelayKind,
    pub output: StdMutex<BTreeMap<usize, String>>,
    pub final_signal: StdMutex<Option<usize>>,
    pub model_backend: StdMutex<Box<dyn ModelBackend>>,
    pub lifecycle: Lifecycle<RelayEvent, RelayResult>,
}

impl RelayRuntime {
    pub(crate) fn new(access: Arc<TaskAccess>, request: RelayRequest) -> Result<Self, String> {
        let config = Config::load().map_err(|error| format!("failed to load config: {error}"))?;
        let model_backend: Box<dyn ModelBackend> = match config.model.selected.as_str() {
            "deepseek" => {
                let backend = construct_model_backend("deepseek", || {
                    DeepseekBackend::new(config.model.deepseek.clone())
                })?;
                Box::new(backend)
            }
            "glm" => {
                let backend =
                    construct_model_backend("glm", || GlmBackend::new(config.model.glm.clone()))?;
                Box::new(backend)
            }
            other => {
                return Err(format!(
                    "unsupported model.selected value \"{other}\"; expected \"deepseek\" or \"glm\""
                ));
            }
        };
        Ok(Self {
            access,
            signature: request.signature,
            kind: request.kind,
            output: StdMutex::new(BTreeMap::new()),
            final_signal: StdMutex::new(None),
            model_backend: StdMutex::new(model_backend),
            lifecycle: Lifecycle::new(),
        })
    }
}

impl RuntimeTrait for RelayRuntime {
    type Base = Relay;
    type Prepared = ModelResponseStream;

    fn signature(&self) -> &RelaySignature {
        &self.signature
    }

    fn lifecycle(&self) -> &Lifecycle<RelayEvent, RelayResult> {
        &self.lifecycle
    }

    fn on_start(&self) -> ActorStartFuture<'_, Self::Prepared> {
        Box::pin(async move {
            let request = match self.model_request() {
                Ok(request) => request,
                Err(reason) => {
                    Logger::error(format!("relay {} failed: {reason}", &self.signature,));
                    self.finish(RelayResultKind::Failed, reason);
                    return None;
                }
            };
            let responses = {
                let mut backend = self
                    .model_backend
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                backend.request_stream(request)
            };
            match responses {
                Ok(responses) => Some(responses),
                Err(error) => {
                    let reason = format!("model request failed: {error}");
                    Logger::error(format!("relay {} failed: {reason}", &self.signature,));
                    self.finish(RelayResultKind::Failed, reason);
                    None
                }
            }
        })
    }

    fn dispatch(&self, event: RelayEvent) {
        match event {
            RelayEvent::Cancel => {
                if matches!(self.status(), ActorStatus::Complete(_)) {
                    return;
                }
                let output = self.output();
                let output = if output.is_empty() {
                    "relay canceled".to_owned()
                } else {
                    output
                };
                self.finish(RelayResultKind::Canceled, output);
            }
        }
    }

    fn event_loop<'a>(
        &'a self,
        mut event_rx: ActorEventReceiver<RelayEvent>,
        mut close_rx: ActorCloseReceiver,
        mut responses: Self::Prepared,
    ) -> ActorFuture<'a> {
        Box::pin(async move {
            loop {
                self::tokio::select! {
                    _ = close_rx.recv() => break,
                    event = event_rx.recv() => {
                        let Some(event) = event else {
                            break;
                        };
                        self.dispatch(event);
                    }
                    response = responses.recv() => {
                        let Some(response) = response else {
                            if !matches!(
                                self.status(),
                                ActorStatus::Complete(_)
                            ) {
                                let reason = "model stream closed \
                                    before completion".to_owned();
                                Logger::error(format!(
                                    "relay {} failed: {reason}",
                                    &self.signature,
                                ));
                                self.finish(
                                    RelayResultKind::Failed,
                                    reason,
                                );
                            }
                            break;
                        };
                        self.on_model_response(response);
                    }
                }
            }
        })
    }

    fn on_finish(&self, result: RelayResult) {
        self.send_owner_update(ActorStatus::Complete(result));
    }
}

// -- Private -- //

impl RelayRuntime {
    fn on_model_response(&self, response: ModelResponse) {
        if matches!(self.status(), ActorStatus::Complete(_)) {
            Logger::error(format!(
                "relay {} received model response after completion",
                &self.signature,
            ));
            return;
        }
        if !response.complete {
            self.output
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .insert(response.seq, response.content);
            return;
        }
        let Some(output) = self.complete_output(response.seq) else {
            let reason = format!(
                "model stream completed with missing chunks; \
                 expected {}",
                response.seq,
            );
            Logger::error(format!("relay {} failed: {reason}", &self.signature,));
            self.finish(RelayResultKind::Failed, reason);
            return;
        };
        *self
            .final_signal
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(response.seq);
        self.finish_succeed(output);
    }

    fn finish_succeed(&self, output: String) {
        match &self.kind {
            RelayKind::IntentAnalyze => self.finish(RelayResultKind::Succeed, output),
            RelayKind::ToolCallSummarize {
                continuation_cursor: _,
                ..
            } => match Self::extract_summary(&output) {
                Ok(summary) => self.finish(RelayResultKind::Succeed, summary),
                Err(reason) => {
                    Logger::error(format!("relay {} failed: {reason}", &self.signature));
                    self.finish(RelayResultKind::Failed, reason);
                }
            },
        }
    }

    fn extract_summary(raw: &str) -> Result<String, String> {
        let draft = StepDraft::parse(raw)
            .map_err(|error| format!("not a valid tool-call draft: {error}"))?;
        let mut invocations = draft.invocations.into_iter();
        let Some(invocation) = invocations.next() else {
            return Err("no tool call was made".to_owned());
        };
        if invocations.next().is_some() {
            return Err("more than one tool call was made".to_owned());
        }
        if invocation.name != WorkflowCallSummary::NAME {
            return Err(format!("unexpected tool call `{}`", invocation.name));
        }
        let tool = WorkflowCallSummary::parse(&invocation.input)
            .map_err(|error| format!("`{}` arguments are invalid: {error}", invocation.name))?;
        Ok(tool.summary)
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

    fn output(&self) -> String {
        self.output
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .values()
            .cloned()
            .collect()
    }

    fn finish(&self, kind: RelayResultKind, output: String) {
        RuntimeTrait::finish(self, RelayResult { kind, output });
    }

    fn send_owner_update(&self, status: ActorStatus<RelayResult>) {
        let event = match &self.kind {
            RelayKind::IntentAnalyze => {
                let intent = self.signature.intent.clone();
                SessionEvent::Task(
                    intent.task.clone(),
                    TaskEvent::Intent(
                        intent,
                        IntentEvent::RelayUpdate(self.signature.clone(), status),
                    ),
                )
            }
            RelayKind::ToolCallSummarize {
                invocation,
                continuation_cursor: _,
                ..
            } => SessionEvent::Task(
                invocation.step.intent.task.clone(),
                TaskEvent::Invocation(
                    invocation.clone(),
                    InvocationEvent::SummarizeUpdate(self.signature.clone(), status),
                ),
            ),
        };
        if self.access.session_tx.send(event).is_err() {
            Logger::warning(format!(
                "relay {} event send failed: session stopped",
                &self.signature,
            ));
        }
    }
}

#[allow(dead_code)]
fn assert_runtime_object_safe(
    runtime: &dyn RuntimeTrait<Base = Relay, Prepared = ModelResponseStream>,
) {
    let _ = runtime.run();
}

fn construct_model_backend<B>(
    backend_name: &str,
    build: impl FnOnce() -> B + std::panic::UnwindSafe,
) -> Result<B, String> {
    std::panic::catch_unwind(build).map_err(|payload| {
        let detail = if let Some(message) = payload.downcast_ref::<String>() {
            message.clone()
        } else if let Some(message) = payload.downcast_ref::<&str>() {
            (*message).to_owned()
        } else {
            "unknown backend construction panic".to_owned()
        };
        format!("failed to construct {backend_name} model backend: {detail}")
    })
}
