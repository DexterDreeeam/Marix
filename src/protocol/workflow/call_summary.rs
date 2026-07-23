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
            description: "Summarize the tool call result presented in the \
                          [TOOL CALL] message. Preserve every detail that \
                          matters for the current task and discard the \
                          rest. Never call it otherwise. MUST CALL \
                          workflow_call_summary whenever a [TOOL CALL] message \
                          is present."
                .to_owned(),
            category: ToolCategory::Workflow,
            system: System {
                platform: Platform::All,
                arch: Arch::All,
            },
            input: r#"{"type":"object","properties":{"summary":{"type":"string","description":"The preserved information from the tool call result, written concisely. Use an empty string when nothing is worth keeping."},"continuation_cursor":{"type":"string","minLength":1,"description":"Return the cursor unchanged only when the [TOOL CALL] provides one and later truncated content may be valuable to the current task; otherwise omit this field."}},"required":["summary"],"additionalProperties":false}"#.to_owned(),
        }
    }

    fn parse(arguments: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(arguments)
    }
}
