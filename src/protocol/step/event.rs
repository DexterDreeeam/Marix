use crate::external::*;

use crate::StepResult;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepEvent {
    Trigger,
    Started,
    Update {
        seq: usize,
        content: String,
    },
    Complete {
        seq_count: usize,
        result: StepResult,
    },
    Fail {
        result: StepResult,
    },
}
