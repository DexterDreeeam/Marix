use marix_common::external::*;
use marix_protocol::{IntentDraft, StepDraft};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StageResultType {
    Plan,
    Reject,
    Infeasible,
    IntentComplete,
    InvocationContinue,
    InvocationComplete,
    NativeToolCalls,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PlanStageResult {
    pub reason: String,
    pub subintents: Vec<IntentDraft>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RejectStageResult {
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InfeasibleStageResult {
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IntentCompleteStageResult {
    pub reason: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InvocationContinueStageResult {
    pub summary: String,
    pub continuation_cursor: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InvocationCompleteStageResult {
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "result",
    content = "payload",
    rename_all = "snake_case"
)]
pub(crate) enum StageResult {
    Plan(PlanStageResult),
    Reject(RejectStageResult),
    Infeasible(InfeasibleStageResult),
    IntentComplete(IntentCompleteStageResult),
    InvocationContinue(InvocationContinueStageResult),
    InvocationComplete(InvocationCompleteStageResult),
    NativeToolCalls(StepDraft),
}

impl StageResult {
    pub(crate) fn result_type(&self) -> StageResultType {
        match self {
            Self::Plan(_) => StageResultType::Plan,
            Self::Reject(_) => StageResultType::Reject,
            Self::Infeasible(_) => StageResultType::Infeasible,
            Self::IntentComplete(_) => StageResultType::IntentComplete,
            Self::InvocationContinue(_) => {
                StageResultType::InvocationContinue
            }
            Self::InvocationComplete(_) => {
                StageResultType::InvocationComplete
            }
            Self::NativeToolCalls(_) => StageResultType::NativeToolCalls,
        }
    }
}
