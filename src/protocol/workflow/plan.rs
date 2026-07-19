use marix_common::{Arch, Platform, System};

use crate::external::*;
use crate::{PlanDraft, ToolCategory, ToolPreview};

use super::WorkflowTool;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkflowPlan {
    pub draft: PlanDraft,
}

impl WorkflowTool for WorkflowPlan {
    const NAME: &'static str = "workflow_plan";

    fn preview() -> ToolPreview {
        ToolPreview {
            name: Self::NAME.to_owned(),
            description: "Create a new plan for the current task with \
                          ordered immutable subtask goals. When \
                          plan_failures exist, choose different goals based \
                          on that failure history."
                .to_owned(),
            category: ToolCategory::Workflow,
            system: System {
                platform: Platform::All,
                arch: Arch::All,
            },
            input: r#"{"type":"object","properties":{"goals":{"type":"array","description":"Ordered immutable subtask goals.","items":{"type":"string","minLength":1,"description":"A simple immutable subtask goal that does not request tool use."},"minItems":2}},"required":["goals"],"additionalProperties":false}"#.to_owned(),
        }
    }

    fn parse(arguments: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(arguments)
    }
}
