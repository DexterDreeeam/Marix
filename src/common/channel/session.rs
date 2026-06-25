use crate::common::external::*;
use crate::common::message::UserMessageEnvelope;

pub(crate) type SessionTaskId = u64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum SessionEvent {
    Accepted,
    Close,
    TaskCreate {
        task_id: SessionTaskId,
        message: UserMessageEnvelope,
    },
    TaskMessage {
        task_id: SessionTaskId,
        message: UserMessageEnvelope,
    },
    TaskCancel {
        task_id: SessionTaskId,
    },
    TaskComplete {
        task_id: SessionTaskId,
    },
}

pub(crate) enum SessionTaskSignal {
    Message(UserMessageEnvelope),
    Cancel,
    Complete,
    Closed,
}
