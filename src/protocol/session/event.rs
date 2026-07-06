use crate::external::*;
use crate::{
    ExecutionEvent, ExecutionSignature, PlanEvent, PlanSignature, StepEvent, StepSignature,
    TaskEvent, TaskSignature,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionEvent {
    Task(TaskSignature, TaskEvent),
    Step(StepSignature, StepEvent),
    Execution(ExecutionSignature, ExecutionEvent),
    Plan(PlanSignature, PlanEvent),
}
