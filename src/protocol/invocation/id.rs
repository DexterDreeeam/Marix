use crate::external::*;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct InvocationId(pub uuid::Uuid);

impl InvocationId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}
