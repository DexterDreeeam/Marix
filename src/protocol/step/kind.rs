use crate::external::*;

use crate::InvocationRequest;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepKind {
    Intent,
    Model(ModelStepKind),
    Invocation(InvocationStepKind),
    User(UserStepKind),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelStepKind {
    Initial,
    Analysis,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvocationStepKind {
    Invocation(InvocationRequest),
    Cancel,
    Kill,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserStepKind {
    Verdict,
    Warrant,
}
