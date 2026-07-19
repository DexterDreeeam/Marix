use crate::external::*;

use super::super::IntentResult;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanResult {
    pub goals: Vec<String>,
    pub results: Vec<Option<IntentResult>>,
    pub reason: String,
}
