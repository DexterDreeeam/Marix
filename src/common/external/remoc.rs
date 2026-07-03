use super::tokio;

pub use ::remoc::rch::base;

pub async fn connect_remoc<SendMessage, ReceiveMessage>(
    socket_rx: tokio::OwnedReadHalf,
    socket_tx: tokio::OwnedWriteHalf,
) -> Result<(base::Sender<SendMessage>, base::Receiver<ReceiveMessage>), String>
where
    SendMessage: ::remoc::RemoteSend,
    ReceiveMessage: ::remoc::RemoteSend,
{
    let (connection, sender, receiver): (
        _,
        base::Sender<SendMessage>,
        base::Receiver<ReceiveMessage>,
    ) = ::remoc::Connect::io(::remoc::Cfg::default(), socket_rx, socket_tx)
        .await
        .map_err(|error| error.to_string())?;
    tokio::spawn(connection);
    Ok((sender, receiver))
}
