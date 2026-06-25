use std::net::SocketAddr;

use crate::common::channel::{ChannelError, SessionEvent};
use crate::common::external::*;
use crate::common::message::UserMessage;

use super::ClientTask;

pub struct ClientSession {
    address: SocketAddr,
    runtime: tokio::Runtime,
    to_agent_tx: Option<remoc::base::Sender<SessionEvent>>,
    event_loop: Option<tokio::JoinHandle<()>>,
}

impl ClientSession {
    pub fn connect(address: SocketAddr) -> Result<Self, ChannelError> {
        let mut session = Self {
            address,
            runtime: Self::build_runtime()?,
            to_agent_tx: None,
            event_loop: None,
        };
        session.connect_agent()?;
        Ok(session)
    }

    pub fn create_task(&mut self, _message: impl UserMessage) -> Result<ClientTask, ChannelError> {
        Err(ChannelError::Unsupported(
            "client task creation is not implemented".to_owned(),
        ))
    }

    pub fn close(&mut self) -> Result<(), ChannelError> {
        let Some(mut to_agent_tx) = self.to_agent_tx.take() else {
            self.abort_event_loop();
            return Ok(());
        };
        let send_result = self.runtime.block_on(to_agent_tx.send(SessionEvent::Close));
        self.abort_event_loop();
        Self::finish_close(send_result)
    }
}

impl ClientSession {
    fn connect_agent(&mut self) -> Result<(), ChannelError> {
        let (to_agent_tx, mut from_agent_rx) = self.runtime.block_on(self.connect_remoc())?;
        self.runtime
            .block_on(Self::wait_for_acceptance(&mut from_agent_rx))?;
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
        self.runtime.spawn(async move {
            while let Ok(Some(SessionEvent::Accepted)) = from_agent_rx.recv().await {}
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
}
