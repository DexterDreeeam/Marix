use marix_common::{Arch, Platform, System};

use crate::external::*;
use crate::{ToolCategory, ToolPreview};

use super::WorkflowTool;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowComplete {
    #[serde(rename = "answer")]
    pub output: String,
}

impl WorkflowTool for WorkflowComplete {
    const NAME: &'static str = "workflow_complete";

    fn preview() -> ToolPreview {
        ToolPreview {
            name: Self::NAME.to_owned(),
            description: "Complete the current task only when its goal has \
                          already been achieved."
                .to_owned(),
            category: ToolCategory::Workflow,
            system: System {
                platform: Platform::All,
                arch: Arch::All,
            },
            input: r#"{"type":"object","properties":{"answer":{"type":"string","minLength":1,"description":"The completed task output."}},"required":["answer"],"additionalProperties":false}"#.to_owned(),
        }
    }

    fn parse(arguments: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(arguments)
    }
}
