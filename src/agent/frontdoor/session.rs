use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{mpsc, Arc, Mutex};

use crate::agent::engine::LoopEngine;
use crate::agent::model::ModelBackendType;
use crate::common::channel::{ChannelError, SessionEvent, SessionTaskId, SessionTaskSignal};
use crate::common::external::*;
use crate::common::message::RequestMessageEnvelope;

use super::task::Task;

pub struct Session {
    bind_address: SocketAddr,
    runtime: Arc<tokio::Runtime>,
    to_client_tx: Option<SharedSessionSender>,
    command_loop: Option<tokio::JoinHandle<()>>,
    task_routes: SharedTaskRoutes,
    engine: Arc<LoopEngine>,
}

impl Session {
    pub fn new(bind_address: SocketAddr) -> Result<Self, ChannelError> {
        Ok(Self {
            bind_address,
            runtime: Arc::new(Self::build_runtime()?),
            to_client_tx: None,
            command_loop: None,
            task_routes: Arc::new(Mutex::new(HashMap::new())),
            engine: Arc::new(
                LoopEngine::new(ModelBackendType::Deepseek)
                    .map_err(|error| ChannelError::InvalidState(format!("{error:?}")))?,
            ),
        })
    }

    pub fn accept(&mut self) -> Result<(), ChannelError> {
        self.clear_finished_client();
        if self.to_client_tx.is_some() {
            return Err(ChannelError::InvalidState(
                "agent session can only accept one client".to_owned(),
            ));
        }

        let (to_client_tx, from_client_rx) = self.runtime.block_on(self.accept_remoc())?;
        let to_client_tx = Arc::new(tokio::Mutex::new(to_client_tx));
        self.runtime
            .block_on(Self::send_event(&to_client_tx, SessionEvent::Accepted))?;

        self.command_loop =
            Some(self.spawn_command_loop(from_client_rx, Arc::clone(&to_client_tx)));
        self.to_client_tx = Some(to_client_tx);
        Ok(())
    }

    pub fn close(&mut self) -> Result<(), ChannelError> {
        if let Some(to_client_tx) = self.to_client_tx.take() {
            self.runtime
                .block_on(Self::send_close_event(&to_client_tx))?;
        }
        if let Some(command_loop) = self.command_loop.take() {
            command_loop.abort();
        }
        self.drain_accepted_tasks();
        Self::close_all_task_routes(&self.task_routes);
        Ok(())
    }
}

// -- Private -- //

type SharedSessionSender = Arc<tokio::Mutex<remoc::base::Sender<SessionEvent>>>;
type SharedTaskRoutes = Arc<Mutex<HashMap<SessionTaskId, mpsc::Sender<SessionTaskSignal>>>>;

impl Session {
    async fn accept_remoc(
        &self,
    ) -> Result<
        (
            remoc::base::Sender<SessionEvent>,
            remoc::base::Receiver<SessionEvent>,
        ),
        ChannelError,
    > {
        let listener = tokio::TcpListener::bind(self.bind_address).await?;
        let (socket, _) = listener.accept().await?;
        let (socket_rx, socket_tx) = socket.into_split();
        remoc::connect_remoc(socket_rx, socket_tx)
            .await
            .map_err(ChannelError::TransportFailed)
    }

    async fn send_event(
        to_client_tx: &SharedSessionSender,
        event: SessionEvent,
    ) -> Result<(), ChannelError> {
        let mut to_client_tx = to_client_tx.lock().await;
        to_client_tx
            .send(event)
            .await
            .map_err(|error| ChannelError::SendFailed(error.to_string()))
    }

    async fn send_close_event(to_client_tx: &SharedSessionSender) -> Result<(), ChannelError> {
        let mut to_client_tx = to_client_tx.lock().await;
        match to_client_tx.send(SessionEvent::Close).await {
            Ok(()) => Ok(()),
            Err(error) if error.is_disconnected() => Ok(()),
            Err(error) => Err(ChannelError::SendFailed(error.to_string())),
        }
    }

    fn spawn_command_loop(
        &self,
        mut from_client_rx: remoc::base::Receiver<SessionEvent>,
        to_client_tx: SharedSessionSender,
    ) -> tokio::JoinHandle<()> {
        let task_routes = Arc::clone(&self.task_routes);
        let runtime = Arc::clone(&self.runtime);
        let engine = Arc::clone(&self.engine);
        self.runtime.spawn(async move {
            while let Ok(Some(event)) = from_client_rx.recv().await {
                match event {
                    SessionEvent::Accepted => {}
                    SessionEvent::Close => {
                        Self::close_all_task_routes(&task_routes);
                        break;
                    }
                    SessionEvent::TaskCreate { task_id, message } => {
                        if Self::accept_task(
                            &engine,
                            &task_routes,
                            &runtime,
                            &to_client_tx,
                            task_id,
                            message,
                        )
                        .is_err()
                        {
                            break;
                        }
                    }
                    SessionEvent::TaskResponseMessage { .. } => {}
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
                }
            }
        })
    }

    fn build_runtime() -> Result<tokio::Runtime, ChannelError> {
        tokio::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(ChannelError::from)
    }

    fn clear_finished_client(&mut self) {
        let Some(command_loop) = self.command_loop.as_ref() else {
            return;
        };
        if command_loop.is_finished() {
            self.command_loop = None;
            self.to_client_tx = None;
            self.drain_accepted_tasks();
            Self::close_all_task_routes(&self.task_routes);
        }
    }

    fn drain_accepted_tasks(&mut self) {
        // Task execution is started from TaskCreate, so there is no accepted-task queue to drain.
    }

    fn accept_task(
        engine: &Arc<LoopEngine>,
        task_routes: &SharedTaskRoutes,
        runtime: &Arc<tokio::Runtime>,
        to_client_tx: &SharedSessionSender,
        task_id: SessionTaskId,
        message: RequestMessageEnvelope,
    ) -> Result<(), ChannelError> {
        let (task_tx, _task_rx) = mpsc::channel();
        Self::insert_task_route(task_routes, task_id, task_tx)?;
        let task = Task::new(
            task_id,
            message,
            Arc::clone(runtime),
            Arc::clone(to_client_tx),
            Arc::clone(task_routes),
        );
        if let Err(error) = task.run(engine.as_ref()) {
            Self::remove_task_route(task_routes, task_id);
            return Err(ChannelError::InvalidState(format!("{error:?}")));
        }
        Ok(())
    }

    fn insert_task_route(
        task_routes: &SharedTaskRoutes,
        task_id: SessionTaskId,
        task_tx: mpsc::Sender<SessionTaskSignal>,
    ) -> Result<(), ChannelError> {
        task_routes
            .lock()
            .map_err(|_| ChannelError::InvalidState("agent task routes are poisoned".to_owned()))?
            .insert(task_id, task_tx);
        Ok(())
    }

    fn remove_task_route(task_routes: &SharedTaskRoutes, task_id: SessionTaskId) {
        let Ok(mut task_routes) = task_routes.lock() else {
            return;
        };
        task_routes.remove(&task_id);
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
