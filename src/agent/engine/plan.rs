use super::task_context::TaskStep;

/// A model-produced plan: the ordered, human-facing jobs the agent intends to
/// carry out for a task. The model may replan on the next model request when a
/// job hits an unexpected step outcome, producing a fresh Plan that supersedes
/// the previous one. A Plan is a planning-time view shown to the user; it is not
/// persisted in the task brief, which records only executed steps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) jobs: Vec<Job>,
}

/// One human-facing unit of work within a plan, shown to the user. A job groups
/// the executable steps it expands into; the agent executes those steps
/// bottom-up while the plan/job grouping stays a display-time structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) steps: Vec<TaskStep>,
}
