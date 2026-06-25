use std::collections::HashMap;
use std::sync::{mpsc, Arc, Mutex};

use crate::common::channel::ChannelError;
use crate::common::channel::{SessionEvent, SessionTaskId, SessionTaskSignal};
use crate::common::external::*;
use crate::common::message::{UserMessage, UserMessageEnvelope};

type SharedSessionSender = Arc<tokio::Mutex<remoc::base::Sender<SessionEvent>>>;
type SharedTaskRoutes = Arc<Mutex<HashMap<SessionTaskId, mpsc::Sender<SessionTaskSignal>>>>;

pub struct AgentTask {
    task_id: SessionTaskId,
    initial_message: Option<UserMessageEnvelope>,
    runtime: Arc<tokio::Runtime>,
    to_client_tx: SharedSessionSender,
    from_client_rx: mpsc::Receiver<SessionTaskSignal>,
    task_routes: SharedTaskRoutes,
    active: bool,
}

impl AgentTask {
    pub(crate) fn new(
        task_id: SessionTaskId,
        initial_message: UserMessageEnvelope,
        runtime: Arc<tokio::Runtime>,
        to_client_tx: SharedSessionSender,
        from_client_rx: mpsc::Receiver<SessionTaskSignal>,
        task_routes: SharedTaskRoutes,
    ) -> Self {
        Self {
            task_id,
            initial_message: Some(initial_message),
            runtime,
            to_client_tx,
            from_client_rx,
            task_routes,
            active: true,
        }
    }

    pub fn send(&mut self, message: impl UserMessage) -> Result<(), ChannelError> {
        self.ensure_active()?;
        self.send_event(SessionEvent::TaskMessage {
            task_id: self.task_id,
            message: message.into_envelope(),
        })
    }

    pub fn receive(&mut self) -> Result<UserMessageEnvelope, ChannelError> {
        self.ensure_active()?;
        if let Some(message) = self.initial_message.take() {
            return Ok(message);
        }
        match self
            .from_client_rx
            .recv()
            .map_err(|_| ChannelError::Disconnected)?
        {
            SessionTaskSignal::Message(message) => Ok(message),
            SessionTaskSignal::Cancel | SessionTaskSignal::Complete | SessionTaskSignal::Closed => {
                self.active = false;
                self.remove_route()?;
                Err(ChannelError::Disconnected)
            }
        }
    }

    pub fn complete(&mut self) -> Result<(), ChannelError> {
        if !self.active {
            return Ok(());
        }
        let result = self.send_event(SessionEvent::TaskComplete {
            task_id: self.task_id,
        });
        self.active = false;
        self.remove_route()?;
        result
    }
}

impl AgentTask {
    fn send_event(&self, event: SessionEvent) -> Result<(), ChannelError> {
        let send_result = self.runtime.block_on(async {
            let mut to_client_tx = self.to_client_tx.lock().await;
            to_client_tx.send(event).await
        });
        match send_result {
            Ok(()) => Ok(()),
            Err(error) if error.is_disconnected() => Err(ChannelError::Disconnected),
            Err(error) => Err(ChannelError::SendFailed(error.to_string())),
        }
    }

    fn ensure_active(&self) -> Result<(), ChannelError> {
        if self.active {
            Ok(())
        } else {
            Err(ChannelError::InvalidState(
                "agent task is closed".to_owned(),
            ))
        }
    }

    fn remove_route(&self) -> Result<(), ChannelError> {
        self.task_routes
            .lock()
            .map_err(|_| ChannelError::InvalidState("agent task routes are poisoned".to_owned()))?
            .remove(&self.task_id);
        Ok(())
    }
}
