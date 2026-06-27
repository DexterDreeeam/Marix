use crate::common::external::*;
use crate::common::message::{RequestMessageEnvelope, ResponseMessageEnvelope};

pub(crate) type SessionTaskId = u64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum SessionEvent {
    Accepted,
    Close,
    TaskCreate {
        task_id: SessionTaskId,
        message: RequestMessageEnvelope,
    },
    TaskResponseMessage {
        task_id: SessionTaskId,
        message: ResponseMessageEnvelope,
    },
    TaskCancel {
        task_id: SessionTaskId,
    },
    TaskComplete {
        task_id: SessionTaskId,
    },
}

pub(crate) enum SessionTaskSignal {
    ResponseMessage(ResponseMessageEnvelope),
    Cancel,
    Complete,
    Closed,
}
