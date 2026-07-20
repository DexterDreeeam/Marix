use crate::external::*;

use crate::TaskSignature;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRequest {
    pub signature: TaskSignature,
    pub content: String,
    #[serde(default)]
    pub max_completion_time_secs: Option<u64>,
    #[serde(default)]
    pub max_relay_count: Option<u64>,
}
