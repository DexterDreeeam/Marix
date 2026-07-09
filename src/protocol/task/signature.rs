use std::fmt;

use crate::external::*;
use crate::{Signature, TaskId};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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

impl Signature for TaskSignature {
    fn id(&self) -> uuid::Uuid {
        self.id.0
    }
}

impl fmt::Display for TaskSignature {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.id.0)
    }
}
