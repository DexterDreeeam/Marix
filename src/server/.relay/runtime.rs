use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::external::*;
use marix_common::{AsyncReceiver, AsyncSender, Logger, build_async_channel};
use marix_protocol::{
    PlanEvent, RelayError, RelayEvent, RuntimeAsync, SessionEvent, StepEvent, StepletStatus,
    TaskEvent,
};

use super::state::RelayState;
use crate::model::{ModelRequest, ModelResponse};

pub(super) struct RelayRuntime {
    state: Arc<RelayState>,
    relay_rx: StdMutex<Option<AsyncReceiver<RelayEvent>>>,
    close_tx: AsyncSender<()>,
    close_rx: StdMutex<Option<AsyncReceiver<()>>>,
}

impl RelayRuntime {
    pub(super) fn new(state: Arc<RelayState>) -> Self {
        let (close_tx, close_rx) = build_async_channel();
        let relay_rx = state
            .relay_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
        let runtime = Self {
            state,
            relay_rx: StdMutex::new(relay_rx),
            close_tx,
            close_rx: StdMutex::new(Some(close_rx)),
        };
        runtime.send_step_event(StepEvent::Update(StepletStatus::Created));
        runtime
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
        Logger::log(format!("[Model Relay] Prompt:\n{}", request.prompt));
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
                self.send_step_event(StepEvent::Update(StepletStatus::Failed));
                self.close();
                return;
            }
        };
        self.send_step_event(StepEvent::Update(StepletStatus::Started));
        Logger::debug(format!(
            "relay {} runtime loop starting",
            &self.state.signature,
        ));
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
        Logger::debug(format!(
            "relay {} runtime loop stopped",
            &self.state.signature,
        ));
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
                self.send_step_event(StepEvent::Update(StepletStatus::Canceled));
                self.close();
                Err(RelayError::Canceled)
            }
        }
    }
}

// -- Private -- //

impl RelayRuntime {
    fn send_step_event(&self, event: StepEvent) {
        let event = SessionEvent::Task(
            self.state.signature.task.clone(),
            TaskEvent::Plan(
                self.state.signature.plan.clone(),
                PlanEvent::Step(self.state.signature.step.clone(), event),
            ),
        );
        if self.state.access.session_tx.send(event).is_err() {
            Logger::warning(format!(
                "relay {} step event failed: session worker stopped",
                &self.state.signature,
            ));
        }
    }

    fn on_model_response(&self, response: ModelResponse) -> Result<(), RelayError> {
        if !response.complete {
            self.state
                .output
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .insert(response.seq, response.content.clone());
            self.send_step_event(StepEvent::Processing {
                seq: response.seq,
                content: response.content,
            });
            return Ok(());
        }

        let first_complete = {
            let mut final_signal = self
                .state
                .final_signal
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let first_complete = final_signal.is_none();
            *final_signal = Some(response.seq);
            first_complete
        };
        let is_complete = Self::is_complete(&self.state);
        if first_complete {
            if !is_complete {
                Logger::warning(format!(
                    "relay {} model completed before all chunks arrived; \
                     expected {}, current output may be incomplete",
                    &self.state.signature, response.seq,
                ));
            }
            Logger::log(format!("[Model Relay] Output:\n{}", self.state.output(),));
        }
        if is_complete {
            self.send_step_event(StepEvent::Update(StepletStatus::Succeed {
                seq_count: response.seq,
            }));
            self.close();
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
