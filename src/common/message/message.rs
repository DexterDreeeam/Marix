use crate::common::external::*;

use super::chat::{ChatRequest, ChatResponseSegment};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestMessageType {
    Chat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseMessageType {
    ChatSegment,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RequestMessageEnvelope {
    ChatRequest(ChatRequest),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResponseMessageEnvelope {
    ChatResponseSegment(ChatResponseSegment),
}

pub trait RequestMessage {
    fn message_type(&self) -> RequestMessageType;

    fn into_envelope(self) -> RequestMessageEnvelope
    where
        Self: Sized;
}

pub trait ResponseMessage {
    fn message_type(&self) -> ResponseMessageType;

    fn into_envelope(self) -> ResponseMessageEnvelope
    where
        Self: Sized;
}

// -- Private -- //

impl RequestMessage for RequestMessageEnvelope {
    fn message_type(&self) -> RequestMessageType {
        match self {
            Self::ChatRequest(message) => message.message_type(),
        }
    }

    fn into_envelope(self) -> RequestMessageEnvelope {
        self
    }
}

impl ResponseMessage for ResponseMessageEnvelope {
    fn message_type(&self) -> ResponseMessageType {
        match self {
            Self::ChatResponseSegment(message) => message.message_type(),
        }
    }

    fn into_envelope(self) -> ResponseMessageEnvelope {
        self
    }
}
