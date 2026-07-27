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
        /// Summaries already collected from earlier chunks of the same
        /// tool output; empty for the first chunk and for unchunked calls.
        #[serde(default)]
        previous_summaries: Vec<String>,
    },
}
