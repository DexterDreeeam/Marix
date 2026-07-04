use crate::external::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepKind {
    Model(ModelStepKind),
    Execution(ExecutionStepKind),
    User(UserStepKind),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelStepKind {
    Initial,
    JobPlan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionStepKind {
    Invocation,
    Cancel,
    Kill,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserStepKind {
    Verdict,
    Warrant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepEvent {
    Started,
    Update { seq: usize, content: String },
    Complete { seq_count: usize },
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepSignature {
    pub step_no: usize,
    pub name: String,
    pub kind: StepKind,
}
