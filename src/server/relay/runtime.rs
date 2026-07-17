use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::external::*;
use marix_common::{
    ActorCloseReceiver, ActorEventReceiver, ActorFuture, ActorStartFuture, ActorStatus, Config,
    Lifecycle, Logger, ModelBackend as ConfigModelBackend, Runtime as RuntimeTrait,
};
use marix_protocol::{
    ContextChain, IntentEvent, RelayEvent, RelayRequest, RelayResult, RelayResultKind,
    RelaySignature, SessionEvent, TaskEvent,
};

use super::Relay;
use crate::model::{
    DeepseekBackend, ModelBackend, ModelRequest, ModelResponse, ModelResponseAsyncReceiver,
};
use crate::prompt::Prompt;
use crate::task::TaskAccess;

pub struct RelayRuntime {
    pub access: Arc<TaskAccess>,
    pub signature: RelaySignature,
    pub prompt: String,
    pub output: StdMutex<BTreeMap<usize, String>>,
    pub final_signal: StdMutex<Option<usize>>,
    pub model_backend: StdMutex<Box<dyn ModelBackend>>,
    pub lifecycle: Lifecycle<RelayEvent, RelayResult>,
}

impl RelayRuntime {
    pub(crate) fn new(access: Arc<TaskAccess>, request: RelayRequest) -> Result<Self, String> {
        let config = Config::load().map_err(|error| format!("failed to load config: {error}"))?;
        let model_backend: Box<dyn ModelBackend> = match config.model.backend {
            ConfigModelBackend::Deepseek => {
                let backend =
                    std::panic::catch_unwind(DeepseekBackend::new).map_err(|payload| {
                        let detail = if let Some(message) = payload.downcast_ref::<String>() {
                            message.clone()
                        } else if let Some(message) = payload.downcast_ref::<&str>() {
                            (*message).to_owned()
                        } else {
                            "unknown backend construction panic".to_owned()
                        };
                        format!("failed to construct model backend: {detail}")
                    })?;
                Box::new(backend)
            }
        };
        Ok(Self {
            access,
            signature: request.signature,
            prompt: request.prompt,
            output: StdMutex::new(BTreeMap::new()),
            final_signal: StdMutex::new(None),
            model_backend: StdMutex::new(model_backend),
            lifecycle: Lifecycle::new(),
        })
    }
}

impl RuntimeTrait for RelayRuntime {
    type Base = Relay;
    type Prepared = ModelResponseAsyncReceiver;

    fn signature(&self) -> &RelaySignature {
        &self.signature
    }

    fn lifecycle(&self) -> &Lifecycle<RelayEvent, RelayResult> {
        &self.lifecycle
    }

    fn on_start(&self) -> ActorStartFuture<'_, Self::Prepared> {
        Box::pin(async move {
            let context = match self.signature.plan.as_ref() {
                Some(plan) => self.access.get_context_chain(plan),
                None => self.access.get_context_chain(&self.signature.intent),
            };
            let context = match context {
                Ok(context) => context,
                Err(reason) => {
                    Logger::error(format!("relay {} failed: {reason}", &self.signature,));
                    self.finish(RelayResultKind::Failed, reason);
                    return None;
                }
            };
            let request = match self.model_request(context) {
                Ok(request) => request,
                Err(reason) => {
                    Logger::error(format!("relay {} failed: {reason}", &self.signature,));
                    self.finish(RelayResultKind::Failed, reason);
                    return None;
                }
            };
            Logger::log(format!(
                "[Model Relay] System:\n{}\n\n\
                 [Model Relay] Context:\n{}\n\n\
                 [Model Relay] Prompt:\n{}",
                request.system, request.context, request.prompt,
            ));
            Logger::debug(format!(
                "relay {} model request includes {} tool(s)",
                &self.signature,
                request.tools.len(),
            ));
            let responses = {
                let mut backend = self
                    .model_backend
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                backend.request_async(request)
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
                if self.status() == ActorStatus::Complete {
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
                            if self.status() != ActorStatus::Complete {
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

    fn on_finish(&self) {
        self.send_owner_update(ActorStatus::Complete);
    }
}

// -- Private -- //

impl RelayRuntime {
    fn model_request(&self, context: ContextChain) -> Result<ModelRequest, String> {
        let session_context = self.access.session_context()?;
        let (task, tools) = {
            let context = session_context
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            (
                context.tasks.with(&self.access.signature, Clone::clone),
                context.tools.clone(),
            )
        };
        let task = task.ok_or_else(|| {
            format!(
                "current task {} was not found in session context",
                &self.access.signature,
            )
        })?;
        let task_preview = serde_json::to_string(&task.preview())
            .map_err(|error| format!("failed to serialize current task preview: {error}"))?;
        let mut system =
            std::panic::catch_unwind(|| Prompt::load("System")).map_err(|payload| {
                let detail = if let Some(message) = payload.downcast_ref::<String>() {
                    message.clone()
                } else if let Some(message) = payload.downcast_ref::<&str>() {
                    (*message).to_owned()
                } else {
                    "unknown prompt loading panic".to_owned()
                };
                format!("failed to load System prompt: {detail}")
            })?;
        system.inject("task_preview".to_owned(), task_preview);
        let system = system
            .prompt()
            .map_err(|error| format!("failed to render System prompt: {error}"))?;
        Ok(ModelRequest {
            relay: self.signature.clone(),
            system,
            context,
            prompt: self.prompt.clone(),
            tools,
        })
    }

    fn on_model_response(&self, response: ModelResponse) {
        if self.status() == ActorStatus::Complete {
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
        Logger::log(format!("[Model Relay] Output:\n{output}"));
        self.finish(RelayResultKind::Succeed, output);
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

    fn send_owner_update(&self, status: ActorStatus) {
        let intent = self.signature.intent.clone();
        let event = SessionEvent::Task(
            intent.task.clone(),
            TaskEvent::Intent(
                intent,
                IntentEvent::RelayUpdate(self.signature.clone(), status),
            ),
        );
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
    runtime: &dyn RuntimeTrait<Base = Relay, Prepared = ModelResponseAsyncReceiver>,
) {
    let _ = runtime.run();
}
