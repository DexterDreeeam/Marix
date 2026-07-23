use crate::external::*;

use crate::InvocationSignature;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelayKind {
    IntentAnalyze,
    ToolCallSummarize {
        invocation: InvocationSignature,
        tool: String,
        output: String,
        #[serde(default)]
        continuation_cursor: Option<String>,
    },
}
