use marix_common::{ExeId, SessionEvent};

pub struct Execution {
    pub id: ExeId,
    pub tool_name: String,
    pub latest_event: Option<SessionEvent>,
}
