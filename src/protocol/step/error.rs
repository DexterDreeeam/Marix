use crate::external::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepError {
    Canceled,
    InvocationCanceled,
    InvocationSucceeded,
    InvocationFailed,
    RelayCanceled,
    RelaySucceeded,
    RelayFailed,
}
