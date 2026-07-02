use std::collections::HashMap;
use std::sync::{mpsc, Arc, Mutex};

use crate::common::channel::ChannelError;
use crate::common::channel::{SessionEvent, SessionTaskId, TaskEvent};
use crate::common::external::*;
use crate::common::message::ResponseMessageEnvelope;

use crate::client::executor::Executor;

pub struct ClientTask {
    task_id: SessionTaskId,
    runtime: Arc<tokio::Runtime>,
    to_agent_tx: SharedSessionSender,
    from_agent_rx: mpsc::Receiver<TaskEvent>,
    task_routes: SharedTaskRoutes,
    executor: SharedExecutor,
    active: bool,
}

impl ClientTask {
    pub(crate) fn new(
        task_id: SessionTaskId,
        runtime: Arc<tokio::Runtime>,
        to_agent_tx: SharedSessionSender,
        from_agent_rx: mpsc::Receiver<TaskEvent>,
        task_routes: SharedTaskRoutes,
        executor: SharedExecutor,
    ) -> Self {
        Self {
            task_id,
            runtime,
            to_agent_tx,
            from_agent_rx,
            task_routes,
            executor,
            active: true,
        }
    }

    pub fn receive(&mut self) -> Result<ResponseMessageEnvelope, ChannelError> {
        self.ensure_active()?;
        match self
            .from_agent_rx
            .recv()
            .map_err(|_| ChannelError::Disconnected)?
        {
            TaskEvent::ResponseMessage(message) => Ok(message),
            TaskEvent::ToolInvocation(invocation) => {
                self.dispatch_tool_invocation(invocation)?;
                self.receive()
            }
            TaskEvent::ToolQuery(signature) => {
                self.dispatch_tool_query(signature)?;
                self.receive()
            }
            TaskEvent::Complete | TaskEvent::Closed => {
                self.active = false;
                self.remove_route()?;
                Err(ChannelError::Disconnected)
            }
            _ => panic!("unexpected task event received by client task"),
        }
    }

    pub fn cancel(&mut self) -> Result<(), ChannelError> {
        if !self.active {
            return Ok(());
        }
        let result = self.send_event(SessionEvent::TaskCancel {
            task_id: self.task_id,
        });
        self.active = false;
        self.remove_route()?;
        result
    }

}

// -- Private -- //

type SharedSessionSender = Arc<tokio::Mutex<remoc::base::Sender<SessionEvent>>>;
type SharedTaskRoutes = Arc<Mutex<HashMap<SessionTaskId, mpsc::Sender<TaskEvent>>>>;
type SharedExecutor = Arc<Mutex<Executor>>;

impl ClientTask {
    fn send_event(&self, event: SessionEvent) -> Result<(), ChannelError> {
        let send_result = self.runtime.block_on(async {
            let mut to_agent_tx = self.to_agent_tx.lock().await;
            to_agent_tx.send(event).await
        });
        match send_result {
            Ok(()) => Ok(()),
            Err(error) if error.is_disconnected() => Err(ChannelError::Disconnected),
            Err(error) => Err(ChannelError::SendFailed(error.to_string())),
        }
    }

    fn send_task_event(&self, _event: TaskEvent) -> Result<(), ChannelError> {
        panic!("not implemented")
    }

    fn dispatch_tool_invocation(
        &mut self,
        _invocation: crate::common::protocol::ToolInvocation,
    ) -> Result<(), ChannelError> {
        panic!("not implemented")
    }

    fn dispatch_tool_query(
        &mut self,
        _signature: crate::common::protocol::ToolSignature,
    ) -> Result<(), ChannelError> {
        panic!("not implemented")
    }

    fn ensure_active(&self) -> Result<(), ChannelError> {
        if self.active {
            Ok(())
        } else {
            Err(ChannelError::InvalidState(
                "client task is closed".to_owned(),
            ))
        }
    }

    fn remove_route(&self) -> Result<(), ChannelError> {
        self.task_routes
            .lock()
            .map_err(|_| ChannelError::InvalidState("client task routes are poisoned".to_owned()))?
            .remove(&self.task_id);
        Ok(())
    }
}
