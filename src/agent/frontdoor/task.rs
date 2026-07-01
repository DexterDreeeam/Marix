use std::collections::HashMap;
use std::sync::{mpsc, Arc, Mutex};

use crate::common::channel::ChannelError;
use crate::common::channel::{SessionEvent, SessionTaskId, SessionTaskSignal};
use crate::common::external::*;
use crate::common::message::{RequestMessageEnvelope, ResponseMessage};

pub struct Task {
    task_id: SessionTaskId,
    initial_message: Option<RequestMessageEnvelope>,
    runtime: Arc<tokio::Runtime>,
    to_client_tx: SharedSessionSender,
    task_routes: SharedTaskRoutes,
    active: bool,
}

impl Task {
    pub(crate) fn new(
        task_id: SessionTaskId,
        initial_message: RequestMessageEnvelope,
        runtime: Arc<tokio::Runtime>,
        to_client_tx: SharedSessionSender,
        task_routes: SharedTaskRoutes,
    ) -> Self {
        Self {
            task_id,
            initial_message: Some(initial_message),
            runtime,
            to_client_tx,
            task_routes,
            active: true,
        }
    }

    pub(crate) fn task_id(&self) -> SessionTaskId {
        self.task_id
    }

    pub fn send(&mut self, message: impl ResponseMessage) -> Result<(), ChannelError> {
        self.ensure_active()?;
        self.send_event(SessionEvent::TaskResponseMessage {
            task_id: self.task_id,
            message: message.into_envelope(),
        })
    }

    pub(crate) fn take_initial_request(&mut self) -> Result<RequestMessageEnvelope, ChannelError> {
        self.ensure_active()?;
        self.initial_message
            .take()
            .ok_or_else(|| ChannelError::InvalidState("task initial request is missing".to_owned()))
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

// -- Private -- //

type SharedSessionSender = Arc<tokio::Mutex<remoc::base::Sender<SessionEvent>>>;
type SharedTaskRoutes = Arc<Mutex<HashMap<SessionTaskId, mpsc::Sender<SessionTaskSignal>>>>;

impl Task {
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
            Err(ChannelError::InvalidState("task is closed".to_owned()))
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
