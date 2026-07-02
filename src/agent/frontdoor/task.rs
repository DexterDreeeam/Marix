use std::collections::HashMap;
use std::sync::{mpsc, Arc, Mutex};

use crate::common::channel::ChannelError;
use crate::common::channel::{SessionEvent, SessionTaskId, TaskEvent};
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
        self.send_event(TaskEvent::ResponseMessage(message.into_envelope()))
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
        let result = self.send_event(TaskEvent::Complete);
        self.active = false;
        self.remove_route()?;
        result
    }
}

// -- Private -- //

type SharedSessionSender = Arc<tokio::Mutex<remoc::base::Sender<SessionEvent>>>;
type SharedTaskRoutes = Arc<Mutex<HashMap<SessionTaskId, mpsc::Sender<TaskEvent>>>>;

impl Task {
    fn send_event(&self, _event: TaskEvent) -> Result<(), ChannelError> {
        panic!("not implemented")
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
