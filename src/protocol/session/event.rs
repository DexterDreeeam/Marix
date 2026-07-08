use crate::external::*;
use crate::{
    ExecutionEvent, ExecutionSignature, PlanEvent, PlanSignature, RelayEvent, RelaySignature,
    StepEvent, StepSignature, TaskEvent, TaskSignature,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionEvent {
    Task(TaskSignature, TaskEvent),
    Step(StepSignature, StepEvent),
    Execution(ExecutionSignature, ExecutionEvent),
    Relay(RelaySignature, RelayEvent),
    Plan(PlanSignature, PlanEvent),
}
