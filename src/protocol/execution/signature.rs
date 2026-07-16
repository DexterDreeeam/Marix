use std::fmt;

use crate::external::*;

use crate::{ExecutionId, InvocationSignature, Signature};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ExecutionSignature {
    pub invocation: InvocationSignature,
    pub execution_id: ExecutionId,
    pub name: String,
}

impl ExecutionSignature {
    pub fn new(invocation: InvocationSignature, name: String) -> Self {
        Self {
            invocation,
            execution_id: ExecutionId::new(),
            name,
        }
    }
}

impl Signature for ExecutionSignature {
    fn type_name(&self) -> &'static str {
        "execution"
    }

    fn id(&self) -> uuid::Uuid {
        self.execution_id.0
    }
}

impl fmt::Display for ExecutionSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.type_name(), self.id())
    }
}
