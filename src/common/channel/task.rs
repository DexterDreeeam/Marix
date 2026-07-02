use crate::common::external::*;
use crate::common::message::ResponseMessageEnvelope;
use crate::common::protocol::{ToolInvocation, ToolExecutionStatus, ToolSignature};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum TaskEvent {
    ResponseMessage(ResponseMessageEnvelope), // To client task.
    ToolInvocation(ToolInvocation),           // To client executor.
    ToolQuery(ToolSignature),                 // To client executor.
    ToolStatus(ToolExecutionStatus),          // To agent task.
    Cancel,   // To agent task.
    Complete, // To client task.
    Closed,   // Local route shutdown.
}
