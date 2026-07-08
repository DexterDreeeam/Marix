use std::net::SocketAddr;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

use crate::config::Config;

const NET_CHANNEL_BUFFER: usize = 16;

pub type Sender<T> = mpsc::Sender<T>;
pub type Receiver<T> = mpsc::Receiver<T>;
pub type NetSender<T> = remoc::rch::mpsc::Sender<T>;
pub type NetReceiver<T> = remoc::rch::mpsc::Receiver<T>;
pub type SharedNetSender<T> = Arc<Mutex<Option<NetSender<T>>>>;
pub type SharedNetReceiver<T> = Arc<Mutex<Option<NetReceiver<T>>>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChannelError {
    Runtime(String),
    Bind(String),
    Accept(String),
    Connect(String),
    Transport(String),
    Setup(String),
    /// The peer rejected the presented token, or the
    /// authentication handshake otherwise failed.
    Auth(String),
}

/// Selects which logical channel a transport call operates on, so the
/// address and handshake token can be resolved from configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelEndpoint {
    Client,
    Host,
    Telemetry,
}

pub fn build_channel<T>() -> (Sender<T>, Receiver<T>) {
    mpsc::channel()
}

/// Accept an inbound connection for the logical channel selected by
/// `endpoint`, run the shared-secret handshake, then return the
/// channel pair.
///
/// The bind address (the node IP plus the port matching `endpoint`)
/// and the expected handshake token are resolved from the loaded
/// configuration inside this function; there is no caller-provided
/// address or token. The peer's presented token is validated against
/// the configured token, and a token mismatch or a failed handshake
/// yields [`ChannelError::Auth`].
pub fn accept_channel<T>(
    endpoint: ChannelEndpoint,
) -> Result<(NetSender<T>, NetReceiver<T>), ChannelError>
where
    T: remoc::RemoteSend + 'static,
    NetSender<T>: Send,
    NetReceiver<T>: Send,
{
    let config = Config::load().map_err(ChannelError::Setup)?;
    let port = match endpoint {
        ChannelEndpoint::Client => config.server.client_port,
        ChannelEndpoint::Host => config.server.host_port,
        ChannelEndpoint::Telemetry => config.server.telemetry_port,
    };
    let address: SocketAddr = format!("{}:{}", config.server.ip, port)
        .parse::<SocketAddr>()
        .map_err(|error| ChannelError::Setup(error.to_string()))?;
    let token = config.server.auth_token;

    let (setup_tx, setup_rx) = mpsc::channel();
    thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                let _ = setup_tx.send(Err(ChannelError::Runtime(error.to_string())));
                return;
            }
        };
        runtime.block_on(async move {
            let listener = match tokio::net::TcpListener::bind(address).await {
                Ok(listener) => listener,
                Err(error) => {
                    let _ = setup_tx.send(Err(ChannelError::Bind(error.to_string())));
                    return;
                }
            };
            let (socket, _) = match listener.accept().await {
                Ok(connection) => connection,
                Err(error) => {
                    let _ = setup_tx.send(Err(ChannelError::Accept(error.to_string())));
                    return;
                }
            };
            let expected_token = token.clone();
            connect_socket(socket, token, Some(expected_token), setup_tx).await;
        });
    });
    setup_rx
        .recv()
        .map_err(|error| ChannelError::Setup(error.to_string()))?
}

/// Connect to the logical channel selected by `endpoint`, run the
/// shared-secret handshake, then return the channel pair.
///
/// The connect address (the node IP plus the port matching
/// `endpoint`) and the handshake token are resolved from the loaded
/// configuration inside this function; there is no caller-provided
/// address or token. The configured token is presented to the
/// server, and if the server rejects it or the handshake fails, the
/// call yields [`ChannelError::Auth`].
pub fn connect_channel<T>(
    endpoint: ChannelEndpoint,
) -> Result<(NetSender<T>, NetReceiver<T>), ChannelError>
where
    T: remoc::RemoteSend + 'static,
    NetSender<T>: Send,
    NetReceiver<T>: Send,
{
    let config = Config::load().map_err(ChannelError::Setup)?;
    let port = match endpoint {
        ChannelEndpoint::Client => config.server.client_port,
        ChannelEndpoint::Host => config.server.host_port,
        ChannelEndpoint::Telemetry => config.server.telemetry_port,
    };
    let address: SocketAddr = format!("{}:{}", config.server.ip, port)
        .parse::<SocketAddr>()
        .map_err(|error| ChannelError::Setup(error.to_string()))?;
    let token = config.server.auth_token;

    let (setup_tx, setup_rx) = mpsc::channel();
    thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                let _ = setup_tx.send(Err(ChannelError::Runtime(error.to_string())));
                return;
            }
        };
        runtime.block_on(async move {
            let socket = match tokio::net::TcpStream::connect(address).await {
                Ok(socket) => socket,
                Err(error) => {
                    let _ = setup_tx.send(Err(ChannelError::Connect(error.to_string())));
                    return;
                }
            };
            connect_socket(socket, token, None, setup_tx).await;
        });
    });
    setup_rx
        .recv()
        .map_err(|error| ChannelError::Setup(error.to_string()))?
}

// -- Private -- //

async fn connect_socket<T>(
    socket: tokio::net::TcpStream,
    token: String,
    expected_token: Option<String>,
    setup_tx: mpsc::Sender<Result<(NetSender<T>, NetReceiver<T>), ChannelError>>,
) where
    T: remoc::RemoteSend + 'static,
    NetSender<T>: Send,
    NetReceiver<T>: Send,
{
    let (socket_rx, socket_tx) = socket.into_split();
    let (connection, mut base_tx, mut base_rx): (
        _,
        remoc::rch::base::Sender<(String, NetReceiver<T>)>,
        remoc::rch::base::Receiver<(String, NetReceiver<T>)>,
    ) = match remoc::Connect::io(remoc::Cfg::default(), socket_rx, socket_tx).await {
        Ok(connection) => connection,
        Err(error) => {
            let _ = setup_tx.send(Err(ChannelError::Transport(error.to_string())));
            return;
        }
    };
    let connection_task = tokio::spawn(connection);
    let (net_tx, net_rx) = remoc::rch::mpsc::channel::<T, _>(NET_CHANNEL_BUFFER);
    if let Err(error) = base_tx.send((token, net_rx)).await {
        let _ = setup_tx.send(Err(ChannelError::Transport(error.to_string())));
        return;
    }
    let (peer_token, peer_rx) = match base_rx.recv().await {
        Ok(Some(payload)) => payload,
        Ok(None) => {
            let _ = setup_tx.send(Err(ChannelError::Transport(
                "peer closed setup channel".to_owned(),
            )));
            return;
        }
        Err(error) => {
            let _ = setup_tx.send(Err(ChannelError::Transport(error.to_string())));
            return;
        }
    };
    if let Some(expected_token) = expected_token
        && peer_token != expected_token
    {
        let _ = setup_tx.send(Err(ChannelError::Auth("channel token mismatch".to_owned())));
        return;
    }
    if setup_tx.send(Ok((net_tx, peer_rx))).is_ok() {
        let _ = connection_task.await;
    }
}
