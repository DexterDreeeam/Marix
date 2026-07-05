use marix_protocol::ExecutionEvent;
use marix_protocol::{StepEvent, StepSignature};

use crate::task::{Task, TaskState};

impl Task {
    pub(super) fn route_execution_session_event(_state: &TaskState, _event: ExecutionEvent) {}

    pub(super) fn route_execution_event(
        _state: &TaskState,
        _signature: &StepSignature,
        _event: StepEvent,
    ) {
    }
}
