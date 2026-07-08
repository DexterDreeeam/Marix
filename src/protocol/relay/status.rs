use crate::external::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelayStatus {
    Started,
    Running,
    Succeed(usize),
    Failed { reason: String },
}
