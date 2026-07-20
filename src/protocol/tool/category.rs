use crate::external::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
// Keep variants alphabetical for stable presentation.
pub enum ToolCategory {
    Coding,
    File,
    Shell,
    System,
    Web,
    Workflow,
}
