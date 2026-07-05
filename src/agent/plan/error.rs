#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    DuplicatePlan,
    PlanNotFound,
    StepNotFound,
}
