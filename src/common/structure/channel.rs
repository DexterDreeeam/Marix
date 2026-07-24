use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex, mpsc as std_mpsc};
use std::thread;
use std::time::Duration;

use crate::Logger;
use crate::config::Config;

pub use crossbeam_channel::select;

const NET_CHANNEL_BUFFER: usize = 16;
/// Maximum time allowed for an outbound TCP connection attempt.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// Wildcard address used by server listeners on every local IPv4 interface.
const SERVER_BIND_IP: Ipv4Addr = Ipv4Addr::UNSPECIFIED;
/// How long the server waits, after a TCP connection is accepted, for
/// that connection to complete the transport handshake and present a
/// valid token before the attempt is abandoned.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
/// How long the server keeps driving a rejected connection so the
/// rejection reaches the connecter before the connection is torn down.
const REJECT_FLUSH_GRACE: Duration = Duration::from_secs(2);
/// How long a connection may sit idle before the OS starts probing it with
/// TCP keepalive, and how the probe/retry cadence is tuned, so a peer that
/// vanished without a clean FIN/RST (for example a VM reboot) is detected
/// well before the accept/connect loop would otherwise wait on it forever.
const KEEPALIVE_IDLE: Duration = Duration::from_secs(10);
const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(5);
const KEEPALIVE_RETRIES: u32 = 3;

pub type Sender<T> = crossbeam_channel::Sender<T>;
pub type Receiver<T> = crossbeam_channel::Receiver<T>;
pub type NetSender<T> = remoc::rch::mpsc::Sender<T>;
pub type NetReceiver<T> = remoc::rch::mpsc::Receiver<T>;
pub type AsyncSender<T> = tokio::sync::mpsc::UnboundedSender<T>;
pub type AsyncReceiver<T> = tokio::sync::mpsc::UnboundedReceiver<T>;
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
    crossbeam_channel::unbounded()
}

pub fn build_async_channel<T>() -> (AsyncSender<T>, AsyncReceiver<T>) {
    tokio::sync::mpsc::unbounded_channel()
}

/// Accept an inbound connection for the logical channel selected by
/// `endpoint`, returning the channel pair for the first connection that
/// both establishes and passes the shared-secret handshake.
///
/// The wildcard bind address (with the port matching `endpoint`) and
/// the expected handshake token are resolved from the loaded
/// configuration inside this function; there is no caller-provided
/// address or token.
///
/// This call blocks until one fully authenticated connection succeeds.
/// A connection that establishes but fails the transport handshake,
/// presents the wrong token, or fails to complete the handshake within
/// [`HANDSHAKE_TIMEOUT`] is abandoned (the connecter is told the token
/// was rejected when applicable) and the listener keeps accepting the
/// next connection. Only server-side setup problems return early:
/// configuration or address resolution failures yield
/// [`ChannelError::Setup`] and a bind failure yields
/// [`ChannelError::Bind`].
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
    let address = SocketAddr::from((SERVER_BIND_IP, port));
    let token = config.server.auth_token;

    let (setup_tx, setup_rx) = std_mpsc::channel();
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
        runtime.block_on(accept_loop::<T>(address, token, setup_tx));
    });
    setup_rx
        .recv()
        .map_err(|error| ChannelError::Setup(error.to_string()))?
}

/// Connect to the logical channel selected by `endpoint`, run the
/// shared-secret handshake, then return the channel pair.
///
/// Uses [`CONNECT_TIMEOUT`] for the TCP connect attempt; see
/// [`connect_channel_with_timeout`] for a caller-supplied timeout.
pub fn connect_channel<T>(
    endpoint: ChannelEndpoint,
) -> Result<(NetSender<T>, NetReceiver<T>), ChannelError>
where
    T: remoc::RemoteSend + 'static,
    NetSender<T>: Send,
    NetReceiver<T>: Send,
{
    connect_channel_with_timeout(endpoint, CONNECT_TIMEOUT)
}

/// Same as [`connect_channel`], but with a caller-supplied timeout for
/// the TCP connect attempt.
///
/// The connect address (the node IP plus the port matching
/// `endpoint`) and the handshake token are resolved from the loaded
/// configuration inside this function; there is no caller-provided
/// address or token. This is a single attempt: it connects once, runs
/// the handshake, and returns. If the server rejects the presented
/// token the call yields [`ChannelError::Auth`]; a failed transport
/// handshake yields [`ChannelError::Transport`].
pub fn connect_channel_with_timeout<T>(
    endpoint: ChannelEndpoint,
    timeout: Duration,
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

    let (setup_tx, setup_rx) = std_mpsc::channel();
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
            let socket = match tokio::time::timeout(
                timeout,
                tokio::net::TcpStream::connect(address),
            )
            .await
            {
                Ok(Ok(socket)) => socket,
                Ok(Err(error)) => {
                    let _ = setup_tx.send(Err(ChannelError::Connect(error.to_string())));
                    return;
                }
                Err(_) => {
                    let _ = setup_tx.send(Err(ChannelError::Connect(format!(
                        "TCP connection timed out after {} ms",
                        timeout.as_millis()
                    ))));
                    return;
                }
            };
            if let Err(error) = arm_tcp_keepalive(&socket) {
                Logger::warning(format!(
                    "failed to arm TCP keepalive on connected socket: {error:?}"
                ));
            }
            connect_attempt(socket, token, setup_tx).await;
        });
    });
    setup_rx
        .recv()
        .map_err(|error| ChannelError::Setup(error.to_string()))?
}

// -- Private -- //

/// Setup-channel handshake messages exchanged over the remoc base
/// channel before the caller-facing pair is handed back.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(bound(
    serialize = "T: remoc::RemoteSend",
    deserialize = "T: remoc::RemoteSend"
))]
enum Handshake<T>
where
    T: remoc::RemoteSend,
{
    /// Connecter -> server: presents the token and the receiver the
    /// server will use to read the connecter's messages.
    Connect { token: String, rx: NetReceiver<T> },
    /// Server -> connecter: token accepted; carries the receiver the
    /// connecter will use to read the server's messages.
    Accept { rx: NetReceiver<T> },
    /// Server -> connecter: token rejected; no pair follows.
    Reject,
}

/// Owns a spawned remoc connection task and aborts it on drop, so a
/// connection that is abandoned (timed out or handshake failed) never
/// leaks a detached driver. Call [`ConnectionGuard::disarm`] on the
/// success path to hand the task off for keep-alive instead.
struct ConnectionGuard {
    handle: Option<tokio::task::JoinHandle<()>>,
}

impl ConnectionGuard {
    fn new(handle: tokio::task::JoinHandle<()>) -> Self {
        Self {
            handle: Some(handle),
        }
    }

    /// Keep driving the connection for up to `grace` (or until it ends
    /// on its own) without disarming, so a pending message can flush
    /// before the guard tears the connection down.
    async fn flush(&mut self, grace: Duration) {
        if let Some(handle) = self.handle.as_mut() {
            let _ = tokio::time::timeout(grace, handle).await;
        }
    }

    fn disarm(mut self) -> tokio::task::JoinHandle<()> {
        self.handle
            .take()
            .expect("connection guard disarmed without a live handle")
    }
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
}

/// Arms OS-level TCP keepalive on `socket` using [`KEEPALIVE_IDLE`],
/// [`KEEPALIVE_INTERVAL`], and [`KEEPALIVE_RETRIES`], so a peer that
/// disappears without sending a clean FIN/RST (for example because its VM
/// rebooted) is detected by the kernel instead of leaving a blocked read
/// pending forever. `pub(super)` only so the sibling integration test module
/// can verify it directly; still fully crate-internal, not part of the
/// crate's public API.
pub(super) fn arm_tcp_keepalive(socket: &tokio::net::TcpStream) -> Result<(), ChannelError> {
    let keepalive = socket2::TcpKeepalive::new()
        .with_time(KEEPALIVE_IDLE)
        .with_interval(KEEPALIVE_INTERVAL)
        .with_retries(KEEPALIVE_RETRIES);
    socket2::SockRef::from(socket)
        .set_tcp_keepalive(&keepalive)
        .map_err(|error| ChannelError::Transport(format!("failed to arm TCP keepalive: {error}",)))
}

/// Bind the listener once, then accept connections until one both
/// establishes and passes the handshake, sending that pair to the
/// caller. Setup failures (bind) are reported to the caller and stop
/// the loop; per-connection failures are abandoned and the loop
/// continues.
async fn accept_loop<T>(
    address: SocketAddr,
    token: String,
    setup_tx: std_mpsc::Sender<Result<(NetSender<T>, NetReceiver<T>), ChannelError>>,
) where
    T: remoc::RemoteSend + 'static,
    NetSender<T>: Send,
    NetReceiver<T>: Send,
{
    let listener = match tokio::net::TcpListener::bind(address).await {
        Ok(listener) => listener,
        Err(error) => {
            let _ = setup_tx.send(Err(ChannelError::Bind(error.to_string())));
            return;
        }
    };

    let (net_tx, peer_rx, connection_handle) = loop {
        let (socket, _) = match listener.accept().await {
            Ok(connection) => connection,
            Err(_) => continue,
        };
        if let Err(error) = arm_tcp_keepalive(&socket) {
            Logger::warning(format!(
                "failed to arm TCP keepalive on accepted socket: {error:?}"
            ));
        }
        match tokio::time::timeout(
            HANDSHAKE_TIMEOUT,
            server_handshake::<T>(socket, token.clone()),
        )
        .await
        {
            Ok(Ok(accepted)) => break accepted,
            Ok(Err(_)) => continue,
            Err(_) => continue,
        }
    };

    // Free the port as soon as one connection is established so a later
    // accept on the same endpoint can bind; the connection itself is
    // kept alive below.
    drop(listener);

    if setup_tx.send(Ok((net_tx, peer_rx))).is_ok() {
        let _ = connection_handle.await;
    } else {
        connection_handle.abort();
    }
}

/// Drive one accepted connection through the transport handshake and
/// token check. On success returns the caller-facing sender/receiver
/// plus the (disarmed) connection task for keep-alive. On any failure
/// returns an error and the internal connection task is aborted; a
/// token mismatch first flushes a [`Handshake::Reject`] to the
/// connecter.
async fn server_handshake<T>(
    socket: tokio::net::TcpStream,
    expected_token: String,
) -> Result<(NetSender<T>, NetReceiver<T>, tokio::task::JoinHandle<()>), ChannelError>
where
    T: remoc::RemoteSend + 'static,
    NetSender<T>: Send,
    NetReceiver<T>: Send,
{
    let (socket_rx, socket_tx) = socket.into_split();
    let (connection, mut base_tx, mut base_rx): (
        _,
        remoc::rch::base::Sender<Handshake<T>>,
        remoc::rch::base::Receiver<Handshake<T>>,
    ) = remoc::Connect::io(remoc::Cfg::default(), socket_rx, socket_tx)
        .await
        .map_err(|error| ChannelError::Transport(error.to_string()))?;
    let mut guard = ConnectionGuard::new(tokio::spawn(async move {
        let _ = connection.await;
    }));

    let message = match base_rx.recv().await {
        Ok(Some(message)) => message,
        Ok(None) => {
            return Err(ChannelError::Transport(
                "connecter closed setup channel".to_owned(),
            ));
        }
        Err(error) => return Err(ChannelError::Transport(error.to_string())),
    };
    let (token, peer_rx) = match message {
        Handshake::Connect { token, rx } => (token, rx),
        _ => {
            return Err(ChannelError::Transport(
                "unexpected handshake from connecter".to_owned(),
            ));
        }
    };

    if token != expected_token {
        let _ = base_tx.send(Handshake::Reject).await;
        drop(base_tx);
        drop(base_rx);
        // Keep the connection running briefly so the rejection reaches
        // the connecter; the guard aborts the task afterwards.
        guard.flush(REJECT_FLUSH_GRACE).await;
        return Err(ChannelError::Auth("channel token rejected".to_owned()));
    }

    let (net_tx, net_rx) = remoc::rch::mpsc::channel::<T, _>(NET_CHANNEL_BUFFER);
    if let Err(error) = base_tx.send(Handshake::Accept { rx: net_rx }).await {
        return Err(ChannelError::Transport(error.to_string()));
    }
    Ok((net_tx, peer_rx, guard.disarm()))
}

/// Connecter side of the handshake: present the token, then interpret
/// the server's accept/reject reply, sending the resulting pair or
/// error back through `setup_tx`.
async fn connect_attempt<T>(
    socket: tokio::net::TcpStream,
    token: String,
    setup_tx: std_mpsc::Sender<Result<(NetSender<T>, NetReceiver<T>), ChannelError>>,
) where
    T: remoc::RemoteSend + 'static,
    NetSender<T>: Send,
    NetReceiver<T>: Send,
{
    match tokio::time::timeout(HANDSHAKE_TIMEOUT, connecter_handshake(socket, token)).await {
        Ok(Ok((net_tx, peer_rx, connection_task))) => {
            if setup_tx.send(Ok((net_tx, peer_rx))).is_ok() {
                let _ = connection_task.await;
            } else {
                connection_task.abort();
            }
        }
        Ok(Err(error)) => {
            let _ = setup_tx.send(Err(error));
        }
        Err(_) => {
            let _ = setup_tx.send(Err(ChannelError::Transport(format!(
                "remoc and token handshake timed out after {} ms",
                HANDSHAKE_TIMEOUT.as_millis()
            ))));
        }
    }
}

async fn connecter_handshake<T>(
    socket: tokio::net::TcpStream,
    token: String,
) -> Result<(NetSender<T>, NetReceiver<T>, tokio::task::JoinHandle<()>), ChannelError>
where
    T: remoc::RemoteSend + 'static,
    NetSender<T>: Send,
    NetReceiver<T>: Send,
{
    let (socket_rx, socket_tx) = socket.into_split();
    let (connection, mut base_tx, mut base_rx): (
        _,
        remoc::rch::base::Sender<Handshake<T>>,
        remoc::rch::base::Receiver<Handshake<T>>,
    ) = remoc::Connect::io(remoc::Cfg::default(), socket_rx, socket_tx)
        .await
        .map_err(|error| ChannelError::Transport(error.to_string()))?;
    let guard = ConnectionGuard::new(tokio::spawn(async move {
        let _ = connection.await;
    }));

    let (net_tx, net_rx) = remoc::rch::mpsc::channel::<T, _>(NET_CHANNEL_BUFFER);
    base_tx
        .send(Handshake::Connect { token, rx: net_rx })
        .await
        .map_err(|error| ChannelError::Transport(error.to_string()))?;

    match base_rx.recv().await {
        Ok(Some(Handshake::Accept { rx: peer_rx })) => Ok((net_tx, peer_rx, guard.disarm())),
        Ok(Some(Handshake::Reject)) => Err(ChannelError::Auth("channel token rejected".to_owned())),
        Ok(Some(_)) => Err(ChannelError::Transport(
            "unexpected handshake from server".to_owned(),
        )),
        Ok(None) => Err(ChannelError::Transport(
            "server closed setup channel".to_owned(),
        )),
        Err(error) => Err(ChannelError::Transport(error.to_string())),
    }
}
