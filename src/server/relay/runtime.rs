use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::external::*;
use marix_common::{AsyncReceiver, AsyncSender, Logger, build_async_channel};
use marix_protocol::{IntentEvent, RelayEvent, RelayStatus, SessionEvent, TaskEvent};

use super::RelayState;
use crate::model::{ModelRequest, ModelResponse};
use crate::task::TaskAccess;

pub struct RelayRuntime {
    pub access: Arc<TaskAccess>,
    pub state: Arc<RelayState>,
    pub relay_rx: StdMutex<Option<AsyncReceiver<RelayEvent>>>,
    pub close_tx: AsyncSender<()>,
    pub close_rx: StdMutex<Option<AsyncReceiver<()>>>,
}

impl RelayRuntime {
    pub async fn run(&self) {
        let Some(mut relay_rx) = self
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
        let Some(mut close_rx) = self
            .close_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
        else {
            self.fail("relay close receiver unavailable".to_owned());
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
                self.fail(format!("model request failed: {error}"));
                return;
            }
        };
        self.set_status(RelayStatus::Started);

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
                            self.fail(
                                "model stream closed before completion"
                                    .to_owned(),
                            );
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
            RelayEvent::Cancel => self.cancel(),
        }
    }
}

// -- Private -- //

impl RelayRuntime {
    pub(crate) fn new(access: Arc<TaskAccess>, state: Arc<RelayState>) -> Self {
        let (close_tx, close_rx) = build_async_channel();
        let relay_rx = state
            .relay_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
        Self {
            access,
            state,
            relay_rx: StdMutex::new(relay_rx),
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
            let mut output = self
                .state
                .output
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if let Some(applied) = output.get(&response.seq) {
                if applied != &response.content {
                    drop(output);
                    self.fail(format!(
                        "model stream sent conflicting output chunk {}",
                        response.seq,
                    ));
                }
                return;
            }
            output.insert(response.seq, response.content);
            return;
        }
        let complete = {
            let output = self
                .state
                .output
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            output.len() == response.seq && (0..response.seq).all(|seq| output.contains_key(&seq))
        };
        if !complete {
            self.fail(format!(
                "model stream completed with missing chunks; expected {}",
                response.seq,
            ));
            return;
        }
        *self
            .state
            .final_signal
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(response.seq);
        self.set_status(RelayStatus::Succeed {
            seq_count: response.seq,
        });
        Logger::log(format!("[Model Relay] Output:\n{}", self.output()));
        self.send_owner_update(RelayStatus::Succeed {
            seq_count: response.seq,
        });
        self.close();
    }

    fn cancel(&self) {
        if self.status().is_terminal() {
            return;
        }
        self.set_status(RelayStatus::Canceled);
        self.send_owner_update(RelayStatus::Canceled);
        self.close();
    }

    fn fail(&self, reason: String) {
        Logger::error(format!("relay {} failed: {reason}", &self.state.signature,));
        self.set_status(RelayStatus::Failed);
        self.send_owner_update(RelayStatus::Failed);
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
        *self
            .state
            .status
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = status;
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

    fn send_owner_update(&self, status: RelayStatus) {
        let intent = self.state.signature.intent.clone();
        let event = SessionEvent::Task(
            intent.task.clone(),
            TaskEvent::Intent(
                intent,
                IntentEvent::RelayUpdate(self.state.signature.clone(), status),
            ),
        );
        if self.access.session_tx.send(event).is_err() {
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
