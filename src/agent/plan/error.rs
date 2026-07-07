#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    DuplicatePlan,
    InvalidModelStep { name: String, input: String },
    InvalidStepKind(String),
    PlanNotFound,
    StepNotFound,
}
