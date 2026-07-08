pub mod channel;
pub mod work_queue;

pub use channel::{
    ChannelEndpoint, ChannelError, NetReceiver, NetSender, Receiver, Sender,
    SharedNetReceiver, SharedNetSender, accept_channel, build_channel,
    connect_channel,
};
pub use work_queue::WorkQueue;
