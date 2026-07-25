use marix_common::{Arch, Platform, System};

use crate::external::*;
use crate::{ToolCategory, ToolPreview};

use super::WorkflowTool;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowCallSummary {
    pub summary: String,
    #[serde(default)]
    pub continuation_cursor: Option<String>,
}

impl WorkflowTool for WorkflowCallSummary {
    const NAME: &'static str = "workflow_call_summary";

    fn preview() -> ToolPreview {
        ToolPreview {
            name: Self::NAME.to_owned(),
            description: "Preserve every important fact relevant to the \
                          current task and discard the rest. Use an empty \
                          summary when nothing is worth keeping. In that \
                          condition, do not call any other tool. Do not call \
                          this tool when the last message does not start with \
                          [TOOL CALL]."
                .to_owned(),
            category: ToolCategory::Workflow,
            system: System {
                platform: Platform::All,
                arch: Arch::All,
            },
            input: r#"{"type":"object","properties":{"summary":{"type":"string","description":"Concise content facts relevant to the current task. State facts directly; never mention or describe tools, tool calls, invocation, execution, returned output, or the summarization process. Use an empty string when nothing is worth keeping."},"continuation_cursor":{"type":"string","minLength":1,"description":"Return the cursor unchanged only when the [TOOL CALL] provides one and later truncated content may be valuable to the current task; otherwise omit this field."}},"required":["summary"],"additionalProperties":false}"#.to_owned(),
        }
    }

    fn parse(arguments: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(arguments)
    }
}
