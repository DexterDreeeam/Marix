use super::{
    RequestMessage, RequestMessageEnvelope, RequestMessageType, ResponseMessage,
    ResponseMessageEnvelope, ResponseMessageType,
};
use crate::common::external::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatRequest {
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatResponseSegment {
    pub content: String,
}

// -- Private -- //

impl RequestMessage for ChatRequest {
    fn message_type(&self) -> RequestMessageType {
        RequestMessageType::Chat
    }

    fn into_envelope(self) -> RequestMessageEnvelope {
        RequestMessageEnvelope::ChatRequest(self)
    }
}

impl ResponseMessage for ChatResponseSegment {
    fn message_type(&self) -> ResponseMessageType {
        ResponseMessageType::ChatSegment
    }

    fn into_envelope(self) -> ResponseMessageEnvelope {
        ResponseMessageEnvelope::ChatResponseSegment(self)
    }
}
