use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::Sender;
use marix_common::external::*;
use marix_protocol::{SessionEvent, TaskSignature};

use crate::session::SessionContext;

#[derive(Clone)]
pub struct TaskAccess {
    pub session_context: Arc<StdMutex<SessionContext>>,
    pub session_tx: Sender<SessionEvent>,
    pub signature: TaskSignature,
    pub user_request: String,
    pub rt: Arc<tokio::Runtime>,
}
