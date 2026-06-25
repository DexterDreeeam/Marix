use crate::common::external::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum SessionEvent {
    Accepted,
    Close,
}
