use crate::external::*;
use crate::protocol::TaskId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskSignature {
    pub name: String,
    pub id: TaskId,
}

impl TaskSignature {
    pub fn new(name: String) -> Self {
        Self {
            name,
            id: TaskId::new(),
        }
    }
}
