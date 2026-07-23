use marix_common::{Arch, Platform, System};

use crate::external::*;
use crate::{ToolCategory, ToolPreview};

use super::WorkflowTool;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowContinuation {
    pub continuation_cursor: String,
}

impl WorkflowTool for WorkflowContinuation {
    const NAME: &'static str = "workflow_continuation";

    fn preview() -> ToolPreview {
        ToolPreview {
            name: Self::NAME.to_owned(),
            description: "Read the next cached output segment when a regular \
                          tool result returns a continuation_cursor. Do not \
                          use this tool for any other purpose."
                .to_owned(),
            category: ToolCategory::Workflow,
            system: System {
                platform: Platform::All,
                arch: Arch::All,
            },
            input: r#"{"type":"object","properties":{"continuation_cursor":{"type":"string","minLength":1,"description":"An opaque continuation cursor that must be returned exactly as received."}},"required":["continuation_cursor"],"additionalProperties":false}"#.to_owned(),
        }
    }

    fn parse(arguments: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(arguments)
    }
}
