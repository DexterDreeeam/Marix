use crate::common::external::*;
use crate::common::message::RequestMessageEnvelope;

pub(crate) type SessionTaskId = u64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum SessionEvent {
    Accepted,
    Close,
    TaskCreate {
        task_id: SessionTaskId,
        message: RequestMessageEnvelope,
    },
    TaskCancel {
        task_id: SessionTaskId,
    },
}
