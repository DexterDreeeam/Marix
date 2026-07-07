use crate::external::*;

use crate::ExecutionRequest;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepKind {
    Intent,
    Model(ModelStepKind),
    Execution(ExecutionStepKind),
    User(UserStepKind),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelStepKind {
    Initial,
    Analysis,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionStepKind {
    Invocation(ExecutionRequest),
    Cancel,
    Kill,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserStepKind {
    Verdict,
    Warrant,
}
