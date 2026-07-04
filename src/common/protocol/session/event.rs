use crate::external::*;
use crate::protocol::{
    ExecutionEvent, ExecutionSignature, StepEvent, StepSignature, TaskEvent, TaskSignature,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionEvent {
    Task(TaskSignature, TaskEvent),
    Step(StepSignature, StepEvent),
    Execution(ExecutionSignature, ExecutionEvent),
}
