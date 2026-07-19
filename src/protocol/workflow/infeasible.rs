use marix_common::{Arch, Platform, System};

use crate::external::*;
use crate::{ToolCategory, ToolPreview};

use super::WorkflowTool;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowInfeasible {
    pub reason: String,
}

impl WorkflowTool for WorkflowInfeasible {
    const NAME: &'static str = "workflow_infeasible";

    fn preview() -> ToolPreview {
        ToolPreview {
            name: Self::NAME.to_owned(),
            description: "Declare the entire task infeasible only after \
                          considering tools, planning, and all plan failures."
                .to_owned(),
            category: ToolCategory::Workflow,
            system: System {
                platform: Platform::All,
                arch: Arch::All,
            },
            input: r#"{"type":"object","properties":{"reason":{"type":"string","minLength":1,"description":"Why the entire task is infeasible."}},"required":["reason"],"additionalProperties":false}"#.to_owned(),
        }
    }

    fn parse(arguments: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(arguments)
    }
}
