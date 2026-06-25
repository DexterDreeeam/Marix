use std::net::SocketAddr;

use crate::common::channel::{ChannelError, SessionEvent};
use crate::common::external::*;

use super::AgentTask;

pub struct AgentSession {
    bind_address: SocketAddr,
    runtime: tokio::Runtime,
    to_client_tx: Option<remoc::base::Sender<SessionEvent>>,
    command_loop: Option<tokio::JoinHandle<()>>,
}

impl AgentSession {
    pub fn new(bind_address: SocketAddr) -> Result<Self, ChannelError> {
        Ok(Self {
            bind_address,
            runtime: Self::build_runtime()?,
            to_client_tx: None,
            command_loop: None,
        })
    }

    pub fn accept(&mut self) -> Result<(), ChannelError> {
        self.clear_finished_client();
        if self.to_client_tx.is_some() {
            return Err(ChannelError::InvalidState(
                "agent session can only accept one client".to_owned(),
            ));
        }

        let (mut to_client_tx, from_client_rx) = self.runtime.block_on(self.accept_remoc())?;
        self.runtime
            .block_on(Self::send_event(&mut to_client_tx, SessionEvent::Accepted))?;

        self.command_loop = Some(self.spawn_command_loop(from_client_rx));
        self.to_client_tx = Some(to_client_tx);
        Ok(())
    }

    pub async fn accept_task(&self) -> Result<AgentTask, ChannelError> {
        Err(ChannelError::Unsupported(
            "agent task acceptance is not implemented".to_owned(),
        ))
    }

    pub fn close(&mut self) -> Result<(), ChannelError> {
        if let Some(mut to_client_tx) = self.to_client_tx.take() {
            self.runtime
                .block_on(Self::send_close_event(&mut to_client_tx))?;
        }
        if let Some(command_loop) = self.command_loop.take() {
            command_loop.abort();
        }
        Ok(())
    }
}

impl AgentSession {
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
        to_client_tx: &mut remoc::base::Sender<SessionEvent>,
        event: SessionEvent,
    ) -> Result<(), ChannelError> {
        to_client_tx
            .send(event)
            .await
            .map_err(|error| ChannelError::SendFailed(error.to_string()))
    }

    async fn send_close_event(
        to_client_tx: &mut remoc::base::Sender<SessionEvent>,
    ) -> Result<(), ChannelError> {
        match to_client_tx.send(SessionEvent::Close).await {
            Ok(()) => Ok(()),
            Err(error) if error.is_disconnected() => Ok(()),
            Err(error) => Err(ChannelError::SendFailed(error.to_string())),
        }
    }

    fn spawn_command_loop(
        &self,
        mut from_client_rx: remoc::base::Receiver<SessionEvent>,
    ) -> tokio::JoinHandle<()> {
        self.runtime.spawn(async move {
            let _ = from_client_rx.recv().await;
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
        }
    }
}
