pub mod channel;
pub mod work_queue;

#[cfg(test)]
mod tests;

pub use channel::{
    AsyncReceiver, AsyncSender, ChannelEndpoint, ChannelError, NetReceiver, NetSender, Receiver,
    Sender, SharedNetReceiver, SharedNetSender, accept_channel, build_async_channel,
    build_channel, connect_channel, select,
};
pub use work_queue::WorkQueue;
