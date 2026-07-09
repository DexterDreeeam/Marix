use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::external::*;
use marix_common::{Logger, build_async_channel};
use marix_protocol::{
    PlanEvent, RelayError, RelayEvent, RelayStatus, RuntimeAsync, SessionEvent, StepEvent,
    TaskEvent,
};

use super::state::RelayState;
use crate::model::{ModelRequest, ModelResponse};

pub(super) struct RelayRuntime {
    state: Arc<RelayState>,
    relay_rx: StdMutex<Option<tokio::mpsc::UnboundedReceiver<RelayEvent>>>,
    close_tx: tokio::mpsc::UnboundedSender<()>,
    close_rx: StdMutex<Option<tokio::mpsc::UnboundedReceiver<()>>>,
}

impl RelayRuntime {
    pub(super) fn new(state: Arc<RelayState>) -> Self {
        let (close_tx, close_rx) = build_async_channel();
        let relay_rx = state
            .relay_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
        Self {
            state,
            relay_rx: StdMutex::new(relay_rx),
            close_tx,
            close_rx: StdMutex::new(Some(close_rx)),
        }
    }

    pub(super) fn send_step_event(state: &RelayState, status: RelayStatus) {
        let event = SessionEvent::Task(
            state.signature.task.clone(),
            TaskEvent::Plan(
                state.signature.plan.clone(),
                PlanEvent::Step(state.signature.step.clone(), StepEvent::RelayUpdate(status)),
            ),
        );
        if state.access.session_tx.send(event).is_err() {
            Logger::warning(format!(
                "relay {} step event failed: session worker stopped",
                &state.signature,
            ));
        }
    }
}

impl RuntimeAsync<RelayEvent, RelayError> for RelayRuntime {
    async fn run(&self) {
        let Some(mut relay_rx) = self
            .relay_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
        else {
            Logger::warning(format!(
                "relay {} runtime stopping: event receiver unavailable",
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
            Logger::warning(format!(
                "relay {} runtime stopping: close receiver unavailable",
                &self.state.signature,
            ));
            return;
        };
        let request = ModelRequest {
            step: self.state.signature.step.clone(),
            prompt: self.state.prompt.clone(),
        };
        let signature = self.state.signature.clone();
        Logger::debug(format!("relay {signature} model async request started"));
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
                Logger::error(format!(
                    "relay {signature} model async request failed: {error}",
                ));
                Self::send_step_event(&self.state, RelayStatus::Failed);
                return;
            }
        };
        Self::send_step_event(&self.state, RelayStatus::Started);
        loop {
            self::tokio::select! {
                _ = close_rx.recv() => break,
                event = relay_rx.recv() => {
                    let Some(event) = event else {
                        break;
                    };
                    if let Err(error) = self.dispatch(event) {
                        Logger::debug(format!(
                            "relay {} runtime stopping: {error:?}",
                            &self.state.signature,
                        ));
                        break;
                    }
                }
                response = responses.recv() => {
                    let Some(response) = response else {
                        break;
                    };
                    if let Err(error) = self.on_model_response(response) {
                        Logger::debug(format!(
                            "relay {} runtime stopping: {error:?}",
                            &self.state.signature,
                        ));
                        break;
                    }
                }
            }
        }
    }

    fn close(&self) {
        if let Err(error) = self.close_tx.send(()) {
            Logger::warning(format!(
                "relay {} close signal failed: {error}",
                &self.state.signature,
            ));
        }
    }

    fn dispatch(&self, event: RelayEvent) -> Result<(), RelayError> {
        match event {
            RelayEvent::Cancel => {
                Self::send_step_event(&self.state, RelayStatus::Canceled);
                Err(RelayError::Canceled)
            }
        }
    }
}

// -- Private -- //

impl RelayRuntime {
    fn on_model_response(&self, response: ModelResponse) -> Result<(), RelayError> {
        if !response.complete {
            self.state
                .output
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .insert(response.seq, response.content.clone());
            Self::send_step_event(
                &self.state,
                RelayStatus::Processing {
                    seq: response.seq,
                    content: response.content,
                },
            );
            return Ok(());
        }

        *self
            .state
            .final_signal
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(response.seq);
        if Self::is_complete(&self.state) {
            Self::send_step_event(
                &self.state,
                RelayStatus::Succeed {
                    seq_count: response.seq,
                },
            );
        }

        Ok(())
    }

    fn is_complete(state: &RelayState) -> bool {
        let final_signal = *state
            .final_signal
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let Some(count) = final_signal else {
            return false;
        };
        state
            .output
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .len()
            == count
    }
}
