#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChannelError {
    Disconnected,
    SendFailed,
    ReceiveFailed,
}
