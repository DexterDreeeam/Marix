use std::{
    net::SocketAddr,
    sync::{Arc, Mutex, mpsc},
};

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

pub fn build_channel<T>() -> (Sender<T>, Receiver<T>) {
    mpsc::channel()
}

/// Accept an inbound connection on `address`, run the shared-secret
/// handshake, then return the channel pair.
///
/// `auth` is the token this side expects; the peer's presented token
/// is validated against it. A token mismatch or a failed handshake
/// yields [`ChannelError::Auth`].
pub fn accept_channel<T>(
    _address: SocketAddr,
    _auth: ChannelAuth,
) -> Result<(NetSender<T>, NetReceiver<T>), ChannelError>
where
    T: remoc::RemoteSend + 'static,
    NetSender<T>: Send,
    NetReceiver<T>: Send,
{
    panic!("not implemented")
}

/// Connect to `address`, run the shared-secret handshake, then
/// return the channel pair.
///
/// `auth` is the token this side presents to the server. If the
/// server rejects it or the handshake fails, the call yields
/// [`ChannelError::Auth`].
pub fn connect_channel<T>(
    _address: SocketAddr,
    _auth: ChannelAuth,
) -> Result<(NetSender<T>, NetReceiver<T>), ChannelError>
where
    T: remoc::RemoteSend + 'static,
    NetSender<T>: Send,
    NetReceiver<T>: Send,
{
    panic!("not implemented")
}

/// Shared-secret token used for the channel authentication
/// handshake.
pub struct ChannelAuth {
    pub token: String,
}
