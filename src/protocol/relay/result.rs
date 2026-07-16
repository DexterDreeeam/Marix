use crate::external::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelayResultKind {
    Succeed,
    Canceled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayResult {
    pub kind: RelayResultKind,
    pub output: String,
}
