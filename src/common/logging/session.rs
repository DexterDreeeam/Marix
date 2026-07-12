use crate::external::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogSession {
    pub id: Option<uuid::Uuid>,
    pub emit_ts: u64,
}
