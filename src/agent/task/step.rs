use marix_common::ExecutionRequest;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StepSequence(pub i32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub sequence: StepSequence,
    pub kind: StepKind,
    pub parameters: ExecutionRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepKind {
    Tool(ToolStepKind),
    Model(ModelStepKind),
    User(UserStepKind),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolStepKind {
    Evoke,
    Query,
    Kill,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelStepKind {
    Initial,
    UserIntentAnalysis,
    ContentSummarization,
    TaskPlanning,
    ResponseComposition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserStepKind {
    Decision,
    Authorize,
}
