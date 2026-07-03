use std::{
    net::SocketAddr,
    sync::{Arc, mpsc},
    thread,
};

use tokio::sync::Mutex;

pub type Sender<T> = mpsc::Sender<T>;
pub type Receiver<T> = mpsc::Receiver<T>;
pub type NetSender<T> = remoc::rch::base::Sender<T>;
pub type NetReceiver<T> = remoc::rch::base::Receiver<T>;
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
}

pub fn build_channel<T>() -> (Sender<T>, Receiver<T>) {
    mpsc::channel()
}

pub fn accept_channel<T>(
    address: SocketAddr,
) -> Result<(NetSender<T>, NetReceiver<T>), ChannelError>
where
    T: remoc::RemoteSend + 'static,
    NetSender<T>: Send,
    NetReceiver<T>: Send,
{
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
            connect_socket(socket, setup_tx).await;
        });
    });
    setup_rx
        .recv()
        .map_err(|error| ChannelError::Setup(error.to_string()))?
}

pub fn connect_channel<T>(
    address: SocketAddr,
) -> Result<(NetSender<T>, NetReceiver<T>), ChannelError>
where
    T: remoc::RemoteSend + 'static,
    NetSender<T>: Send,
    NetReceiver<T>: Send,
{
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
            connect_socket(socket, setup_tx).await;
        });
    });
    setup_rx
        .recv()
        .map_err(|error| ChannelError::Setup(error.to_string()))?
}

// -- Private -- //

async fn connect_socket<T>(
    socket: tokio::net::TcpStream,
    setup_tx: mpsc::Sender<Result<(NetSender<T>, NetReceiver<T>), ChannelError>>,
) where
    T: remoc::RemoteSend + 'static,
    NetSender<T>: Send,
    NetReceiver<T>: Send,
{
    let (socket_rx, socket_tx) = socket.into_split();
    let (connection, sender, receiver): (_, NetSender<T>, NetReceiver<T>) =
        match remoc::Connect::io(remoc::Cfg::default(), socket_rx, socket_tx).await {
            Ok(connection) => connection,
            Err(error) => {
                let _ = setup_tx.send(Err(ChannelError::Transport(error.to_string())));
                return;
            }
        };
    if setup_tx.send(Ok((sender, receiver))).is_ok() {
        let _ = connection.await;
    }
}
