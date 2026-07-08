use crate::external::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanError {
    Canceled,
    DuplicatePlan,
    InvalidModelStep { name: String, input: String },
    InvalidStepKind(String),
    PlanNotFound,
    StepNotFound,
}
