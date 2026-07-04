use crate::external::*;

use crate::protocol::{StepKind, TaskSignature};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepSignature {
    pub task: TaskSignature,
    pub step_no: usize,
    pub name: String,
    pub kind: StepKind,
}

impl StepSignature {
    pub fn new(task: TaskSignature, step_no: usize, name: String, kind: StepKind) -> Self {
        Self {
            task,
            step_no,
            name,
            kind,
        }
    }
}
