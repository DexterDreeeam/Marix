use marix_protocol::ExecutionEvent;

use crate::task::{Task, TaskState};

impl Task {
    pub(super) fn route_execution_event(_state: &TaskState, _event: ExecutionEvent) {}
}
