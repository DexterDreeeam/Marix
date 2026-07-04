use crate::external::*;
use crate::protocol::{ExecutionEvent, ExecutionSignature, StepEvent, TaskEvent, TaskSignature};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionEvent {
    Task(TaskSignature, TaskEvent),
    Step(StepEvent),
    Execution(ExecutionSignature, ExecutionEvent),
}
