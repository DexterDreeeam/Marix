use crate::external::*;

use crate::protocol::ExecutionRequest;

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
    Invocation(ExecutionRequest),
    Cancel,
    Kill,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserStepKind {
    Verdict,
    Warrant,
}
