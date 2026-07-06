use crate::external::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogTag {
    Info,
    Warning,
    Error,
    Debug,
}
