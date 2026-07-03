pub mod chat;

pub use chat::{ChatRequest, ChatResponseSegment};
pub use message::{
    RequestMessage, RequestMessageEnvelope, RequestMessageType, ResponseMessage,
    ResponseMessageEnvelope, ResponseMessageType,
};

// -- Private -- //

mod message;
