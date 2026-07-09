use crate::external::*;

use crate::TaskSignature;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRequest {
    pub signature: TaskSignature,
    pub content: String,
}
