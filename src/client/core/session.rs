use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{mpsc, Arc, Mutex};

use crate::common::channel::{ChannelError, SessionEvent, SessionTaskId, SessionTaskSignal};
use crate::common::external::*;
use crate::common::message::UserMessage;

use super::task::ClientTask;

type SharedSessionSender = Arc<tokio::Mutex<remoc::base::Sender<SessionEvent>>>;
type SharedTaskRoutes = Arc<Mutex<HashMap<SessionTaskId, mpsc::Sender<SessionTaskSignal>>>>;

pub struct ClientSession {
    address: SocketAddr,
    runtime: Arc<tokio::Runtime>,
    to_agent_tx: Option<SharedSessionSender>,
    event_loop: Option<tokio::JoinHandle<()>>,
    task_routes: SharedTaskRoutes,
    next_task_id: SessionTaskId,
}

impl ClientSession {
    pub fn connect(address: SocketAddr) -> Result<Self, ChannelError> {
        let mut session = Self {
            address,
            runtime: Arc::new(Self::build_runtime()?),
            to_agent_tx: None,
            event_loop: None,
            task_routes: Arc::new(Mutex::new(HashMap::new())),
            next_task_id: 1,
        };
        session.connect_agent()?;
        Ok(session)
    }

    pub fn create_task(&mut self, message: impl UserMessage) -> Result<ClientTask, ChannelError> {
        let to_agent_tx = self
            .to_agent_tx
            .as_ref()
            .ok_or(ChannelError::Disconnected)?;
        let task_id = self.next_task_id;
        self.next_task_id = self.next_task_id.checked_add(1).ok_or_else(|| {
            ChannelError::InvalidState("client session task id exhausted".to_owned())
        })?;
        let (task_tx, task_rx) = mpsc::channel();
        self.task_routes
            .lock()
            .map_err(|_| ChannelError::InvalidState("client task routes are poisoned".to_owned()))?
            .insert(task_id, task_tx);

        if let Err(error) = self.send_event(
            to_agent_tx,
            SessionEvent::TaskCreate {
                task_id,
                message: message.into_envelope(),
            },
        ) {
            self.remove_task_route(task_id)?;
            return Err(error);
        }

        Ok(ClientTask::new(
            task_id,
            Arc::clone(&self.runtime),
            Arc::clone(to_agent_tx),
            task_rx,
            Arc::clone(&self.task_routes),
        ))
    }

    pub fn close(&mut self) -> Result<(), ChannelError> {
        let Some(to_agent_tx) = self.to_agent_tx.take() else {
            self.abort_event_loop();
            return Ok(());
        };
        let send_result = self.runtime.block_on(async {
            let mut to_agent_tx = to_agent_tx.lock().await;
            to_agent_tx.send(SessionEvent::Close).await
        });
        self.abort_event_loop();
        Self::close_all_task_routes(&self.task_routes);
        Self::finish_close(send_result)
    }
}

impl ClientSession {
    fn connect_agent(&mut self) -> Result<(), ChannelError> {
        let (to_agent_tx, mut from_agent_rx) = self.runtime.block_on(self.connect_remoc())?;
        self.runtime
            .block_on(Self::wait_for_acceptance(&mut from_agent_rx))?;
        let to_agent_tx = Arc::new(tokio::Mutex::new(to_agent_tx));
        self.event_loop = Some(self.spawn_event_loop(from_agent_rx));
        self.to_agent_tx = Some(to_agent_tx);
        Ok(())
    }

    async fn connect_remoc(
        &self,
    ) -> Result<
        (
            remoc::base::Sender<SessionEvent>,
            remoc::base::Receiver<SessionEvent>,
        ),
        ChannelError,
    > {
        let socket = tokio::TcpStream::connect(self.address).await?;
        let (socket_rx, socket_tx) = socket.into_split();
        remoc::connect_remoc(socket_rx, socket_tx)
            .await
            .map_err(ChannelError::TransportFailed)
    }

    fn spawn_event_loop(
        &self,
        mut from_agent_rx: remoc::base::Receiver<SessionEvent>,
    ) -> tokio::JoinHandle<()> {
        let task_routes = Arc::clone(&self.task_routes);
        self.runtime.spawn(async move {
            while let Ok(Some(event)) = from_agent_rx.recv().await {
                match event {
                    SessionEvent::Accepted => {}
                    SessionEvent::Close => {
                        Self::close_all_task_routes(&task_routes);
                        break;
                    }
                    SessionEvent::TaskMessage { task_id, message } => {
                        Self::route_task_signal(
                            &task_routes,
                            task_id,
                            SessionTaskSignal::Message(message),
                            false,
                        );
                    }
                    SessionEvent::TaskCancel { task_id } => {
                        Self::route_task_signal(
                            &task_routes,
                            task_id,
                            SessionTaskSignal::Cancel,
                            true,
                        );
                    }
                    SessionEvent::TaskComplete { task_id } => {
                        Self::route_task_signal(
                            &task_routes,
                            task_id,
                            SessionTaskSignal::Complete,
                            true,
                        );
                    }
                    SessionEvent::TaskCreate { .. } => {}
                }
            }
        })
    }

    async fn wait_for_acceptance(
        from_agent_rx: &mut remoc::base::Receiver<SessionEvent>,
    ) -> Result<(), ChannelError> {
        match from_agent_rx
            .recv()
            .await
            .map_err(|error| ChannelError::ReceiveFailed(error.to_string()))?
        {
            Some(SessionEvent::Accepted) => Ok(()),
            Some(SessionEvent::Close) | None => Err(ChannelError::Disconnected),
            Some(
                SessionEvent::TaskCreate { .. }
                | SessionEvent::TaskMessage { .. }
                | SessionEvent::TaskCancel { .. }
                | SessionEvent::TaskComplete { .. },
            ) => Err(ChannelError::InvalidState(
                "client received task event before session acceptance".to_owned(),
            )),
        }
    }

    fn build_runtime() -> Result<tokio::Runtime, ChannelError> {
        tokio::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(ChannelError::from)
    }

    fn finish_close(
        result: Result<(), remoc::base::SendError<SessionEvent>>,
    ) -> Result<(), ChannelError> {
        match result {
            Ok(()) => Ok(()),
            Err(error) if error.is_disconnected() => Ok(()),
            Err(error) => Err(ChannelError::SendFailed(error.to_string())),
        }
    }

    fn abort_event_loop(&mut self) {
        if let Some(event_loop) = self.event_loop.take() {
            event_loop.abort();
        }
    }

    fn send_event(
        &self,
        to_agent_tx: &SharedSessionSender,
        event: SessionEvent,
    ) -> Result<(), ChannelError> {
        let send_result = self.runtime.block_on(async {
            let mut to_agent_tx = to_agent_tx.lock().await;
            to_agent_tx.send(event).await
        });
        match send_result {
            Ok(()) => Ok(()),
            Err(error) if error.is_disconnected() => Err(ChannelError::Disconnected),
            Err(error) => Err(ChannelError::SendFailed(error.to_string())),
        }
    }

    fn remove_task_route(&self, task_id: SessionTaskId) -> Result<(), ChannelError> {
        self.task_routes
            .lock()
            .map_err(|_| ChannelError::InvalidState("client task routes are poisoned".to_owned()))?
            .remove(&task_id);
        Ok(())
    }

    fn route_task_signal(
        task_routes: &SharedTaskRoutes,
        task_id: SessionTaskId,
        signal: SessionTaskSignal,
        remove: bool,
    ) {
        let Ok(mut task_routes) = task_routes.lock() else {
            return;
        };
        let task_tx = if remove {
            task_routes.remove(&task_id)
        } else {
            task_routes.get(&task_id).cloned()
        };
        if let Some(task_tx) = task_tx {
            let _ = task_tx.send(signal);
        }
    }

    fn close_all_task_routes(task_routes: &SharedTaskRoutes) {
        let Ok(mut task_routes) = task_routes.lock() else {
            return;
        };
        for (_, task_tx) in task_routes.drain() {
            let _ = task_tx.send(SessionTaskSignal::Closed);
        }
    }
}
