use marix_common::external::*;
use marix_protocol::{IntentDraft, StepDraft};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "result",
    content = "payload",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub(crate) enum StageResult {
    Plan {
        reason: String,
        subintents: Vec<IntentDraft>,
    },
    Reject {
        reason: String,
    },
    Infeasible {
        reason: String,
    },
    IntentComplete {
        reason: String,
        summary: String,
    },
    NativeToolCalls(StepDraft),
}
