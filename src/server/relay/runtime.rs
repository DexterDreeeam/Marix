use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::external::*;
use marix_common::{AsyncReceiver, AsyncSender, Logger, build_async_channel};
use marix_protocol::{
    IntentEvent, RelayEvent, RelayResult, RelayResultKind, RelayStatus, SessionEvent, TaskEvent,
};

use super::RelayState;
use crate::model::{ModelRequest, ModelResponse};

pub struct RelayRuntime {
    pub state: Arc<RelayState>,
    pub close_tx: AsyncSender<()>,
    pub close_rx: StdMutex<Option<AsyncReceiver<()>>>,
}

impl RelayRuntime {
    pub async fn run(&self) {
        let Some(mut relay_rx) = self
            .state
            .relay_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
        else {
            Logger::warning(format!(
                "relay {} start ignored: already running",
                &self.state.signature,
            ));
            return;
        };
        self.set_status(RelayStatus::Running);
        let Some(mut close_rx) = self
            .close_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
        else {
            self.finish(
                RelayResultKind::Failed,
                "relay close receiver unavailable".to_owned(),
            );
            return;
        };
        let request = ModelRequest {
            relay: self.state.signature.clone(),
            prompt: self.state.prompt.clone(),
        };
        Logger::log(format!("[Model Relay] Prompt:\n{}", request.prompt));
        let responses = {
            let mut backend = self
                .state
                .model_backend
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            backend.request_async(request)
        };
        let mut responses = match responses {
            Ok(responses) => responses,
            Err(error) => {
                let reason = format!("model request failed: {error}");
                Logger::error(format!(
                    "relay {} failed: {reason}",
                    &self.state.signature,
                ));
                self.finish(RelayResultKind::Failed, reason);
                return;
            }
        };

        loop {
            self::tokio::select! {
                _ = close_rx.recv() => break,
                event = relay_rx.recv() => {
                    let Some(event) = event else {
                        break;
                    };
                    self.dispatch(event);
                }
                response = responses.recv() => {
                    let Some(response) = response else {
                        if !self.status().is_terminal() {
                            let reason =
                                "model stream closed before completion".to_owned();
                            Logger::error(format!(
                                "relay {} failed: {reason}",
                                &self.state.signature,
                            ));
                            self.finish(RelayResultKind::Failed, reason);
                        }
                        break;
                    };
                    self.on_model_response(response);
                }
            }
        }
    }

    pub fn dispatch(&self, event: RelayEvent) {
        match event {
            RelayEvent::Cancel => {
                if self.status().is_terminal() {
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
}

// -- Private -- //

impl RelayRuntime {
    pub(crate) fn new(state: Arc<RelayState>) -> Self {
        let (close_tx, close_rx) = build_async_channel();
        Self {
            state,
            close_tx,
            close_rx: StdMutex::new(Some(close_rx)),
        }
    }

    fn on_model_response(&self, response: ModelResponse) {
        if self.status().is_terminal() {
            Logger::error(format!(
                "relay {} received model response after completion",
                &self.state.signature,
            ));
            return;
        }
        if !response.complete {
            self.state
                .output
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .insert(response.seq, response.content);
            return;
        }
        let Some(output) = self.complete_output(response.seq) else {
            let reason = format!(
                "model stream completed with missing chunks; expected {}",
                response.seq,
            );
            Logger::error(format!(
                "relay {} failed: {reason}",
                &self.state.signature,
            ));
            self.finish(RelayResultKind::Failed, reason);
            return;
        };
        *self
            .state
            .final_signal
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(response.seq);
        Logger::log(format!("[Model Relay] Output:\n{output}"));
        self.finish(RelayResultKind::Succeed, output);
    }

    fn complete_output(&self, seq_count: usize) -> Option<String> {
        let output = self
            .state
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
        self.state
            .output
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .values()
            .cloned()
            .collect()
    }

    fn finish(&self, kind: RelayResultKind, output: String) {
        let result = RelayResult { kind, output };
        self.set_status(RelayStatus::Complete(result));
        self.close();
    }

    fn status(&self) -> RelayStatus {
        self.state
            .status
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    fn set_status(&self, status: RelayStatus) {
        let mut current = self
            .state
            .status
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if current.is_terminal() {
            return;
        }
        let send_update = matches!(&status, RelayStatus::Complete(_));
        *current = status.clone();
        drop(current);
        if send_update {
            self.send_owner_update(status);
        }
    }

    fn send_owner_update(&self, status: RelayStatus) {
        let intent = self.state.signature.intent.clone();
        let event = SessionEvent::Task(
            intent.task.clone(),
            TaskEvent::Intent(
                intent,
                IntentEvent::RelayUpdate(self.state.signature.clone(), status),
            ),
        );
        if self.state.access.session_tx.send(event).is_err() {
            Logger::warning(format!(
                "relay {} event send failed: session stopped",
                &self.state.signature,
            ));
        }
    }

    fn close(&self) {
        if self.close_tx.send(()).is_err() {
            Logger::warning(format!(
                "relay {} close signal failed",
                &self.state.signature,
            ));
        }
    }
}
